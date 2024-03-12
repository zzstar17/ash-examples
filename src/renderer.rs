use ash::vk;
use std::{
  ops::BitOr,
  ptr::{self, addr_of},
};

use crate::{
  allocator::allocate_and_bind_memory,
  command_pools::CommandPools,
  create_objs::{create_buffer, create_fence, create_image, create_semaphore},
  destroy,
  device::{create_logical_device, PhysicalDevice, Queues},
  device_destroyable::{DeviceManuallyDestroyed, ManuallyDestroyed},
  entry,
  errors::{AllocationError, InitializationError, OutOfMemoryError},
  instance::create_instance,
  utility::OnErr,
};

pub struct Renderer {
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: crate::validation_layers::DebugUtils,
  physical_device: PhysicalDevice,
  device: ash::Device,
  queues: Queues,
  command_pools: CommandPools,
  gpu_data: GPUData,
}

struct GPUData {
  clear_image: vk::Image,
  clear_image_memory: vk::DeviceMemory,
  final_buffer: vk::Buffer,
  final_buffer_size: u64,
  final_buffer_memory: vk::DeviceMemory,
}

impl Renderer {
  pub fn initialize(
    image_width: u32,
    image_height: u32,
    buffer_size: u64,
  ) -> Result<Self, InitializationError> {
    let entry: ash::Entry = unsafe { entry::get_entry() };

    #[cfg(feature = "vl")]
    let (instance, debug_utils) = create_instance(&entry)?;
    #[cfg(not(feature = "vl"))]
    let instance = create_instance(&entry)?;

    let physical_device = match unsafe { PhysicalDevice::select(&instance) }
      .on_err(|_| unsafe { destroy!(&debug_utils, &instance) })?
    {
      Some(device) => device,
      None => {
        unsafe { destroy!(&debug_utils, &instance) };
        return Err(InitializationError::NoCompatibleDevices);
      }
    };

    let (device, queues) = create_logical_device(&instance, &physical_device)
      .on_err(|_| unsafe { destroy!(&debug_utils, &instance) })?;

    let command_pools = CommandPools::new(&device, &physical_device)
      .on_err(|_| unsafe { destroy!(&device, &debug_utils, &instance) })?;

    let gpu_data = GPUData::new(
      &device,
      &physical_device,
      image_width,
      image_height,
      buffer_size,
    )
    .on_err(|_| unsafe { destroy!(&device => &command_pools, &device, &debug_utils, &instance) })?;

    Ok(Self {
      _entry: entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      physical_device,
      device,
      queues,
      command_pools,
      gpu_data,
    })
  }

  pub unsafe fn record_work(&mut self) -> Result<(), OutOfMemoryError> {
    self.command_pools.compute_pool.reset(&self.device)?;
    self.command_pools.compute_pool.record_clear_img(
      &self.device,
      &self.physical_device.queue_families,
      self.gpu_data.clear_image,
    )?;

    self.command_pools.transfer_pool.reset(&self.device)?;
    self.command_pools.transfer_pool.record_copy_img_to_buffer(
      &self.device,
      &self.physical_device.queue_families,
      self.gpu_data.clear_image,
      self.gpu_data.final_buffer,
    )?;

    Ok(())
  }

  // can return vk::Result::ERROR_DEVICE_LOST
  pub fn submit_and_wait(&self) -> Result<(), vk::Result> {
    let image_clear_finished = create_semaphore(&self.device)?;
    let all_done = create_fence(&self.device)
      .on_err(|_| unsafe { destroy!(&self.device => &image_clear_finished) })?;

    let clear_image_submit = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: addr_of!(self.command_pools.compute_pool.clear_img),
      signal_semaphore_count: 1,
      p_signal_semaphores: addr_of!(image_clear_finished),
    };
    let wait_for = vk::PipelineStageFlags::TRANSFER;
    let transfer_image_submit = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 1,
      p_wait_semaphores: addr_of!(image_clear_finished),
      p_wait_dst_stage_mask: addr_of!(wait_for),
      command_buffer_count: 1,
      p_command_buffers: addr_of!(self.command_pools.transfer_pool.copy_image_to_buffer),
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
    };

    let destroy_objs = || unsafe { destroy!(&self.device => &image_clear_finished, &all_done) };

    unsafe {
      self
        .device
        .queue_submit(
          self.queues.compute,
          &[clear_image_submit],
          vk::Fence::null(),
        )
        .on_err(|_| destroy_objs())?;
      self
        .device
        .queue_submit(self.queues.transfer, &[transfer_image_submit], all_done)
        .on_err(|_| destroy_objs())?;

      self
        .device
        .wait_for_fences(&[all_done], true, u64::MAX)
        .on_err(|_| destroy_objs())?;
    }

    destroy_objs();

    Ok(())
  }

  pub unsafe fn get_resulting_data<F: FnOnce(&[u8])>(&self, f: F) -> Result<(), vk::Result> {
    self.gpu_data.get_buffer_data(&self.device, f)
  }
}

impl Drop for Renderer {
  fn drop(&mut self) {
    log::debug!("Destroying renderer objects...");
    unsafe {
      // wait until all operations have finished and the device is safe to destroy
      self
        .device
        .device_wait_idle()
        .expect("Failed to wait for the device to become idle during drop");

      destroy!(&self.device => &self.command_pools, &self.gpu_data);

      ManuallyDestroyed::destroy_self(&self.device);

      #[cfg(feature = "vl")]
      {
        ManuallyDestroyed::destroy_self(&self.debug_utils);
      }
      ManuallyDestroyed::destroy_self(&self.instance);
    }
  }
}

impl GPUData {
  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    image_width: u32,
    image_height: u32,
    buffer_size: u64,
  ) -> Result<Self, AllocationError> {
    // GPU image with DEVICE_LOCAL flags
    let clear_image = create_image(
      &device,
      image_width,
      image_height,
      vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::TRANSFER_DST),
    )?;
    log::debug!("Allocating memory for the image that will be cleared");
    let clear_image_memory = match allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[],
      &[],
      &[clear_image],
      &[unsafe { device.get_image_memory_requirements(clear_image) }],
    )
    .or_else(|err| {
      log::warn!("Failed to allocate optimal memory for image:\n{:?}", err);
      allocate_and_bind_memory(
        &device,
        &physical_device,
        vk::MemoryPropertyFlags::empty(),
        &[],
        &[],
        &[clear_image],
        &[unsafe { device.get_image_memory_requirements(clear_image) }],
      )
    }) {
      Ok(alloc) => alloc.memory,
      Err(err) => {
        unsafe {
          clear_image.destroy_self(device);
        }
        return Err(err);
      }
    };

    let final_buffer = match create_buffer(&device, buffer_size, vk::BufferUsageFlags::TRANSFER_DST)
    {
      Ok(buffer) => buffer,
      Err(err) => {
        unsafe {
          destroy!(device => &clear_image_memory, &clear_image);
        }
        return Err(err.into());
      }
    };
    log::debug!("Allocating memory for the final buffer");
    let final_buffer_memory = match allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
      &[final_buffer],
      &[unsafe { device.get_buffer_memory_requirements(final_buffer) }],
      &[],
      &[],
    )
    .or_else(|err| {
      log::warn!(
        "Failed to allocate optimal memory for the final buffer:\n{:?}",
        err
      );
      allocate_and_bind_memory(
        &device,
        &physical_device,
        vk::MemoryPropertyFlags::HOST_VISIBLE,
        &[final_buffer],
        &[unsafe { device.get_buffer_memory_requirements(final_buffer) }],
        &[],
        &[],
      )
    }) {
      Ok(alloc) => alloc.memory,
      Err(err) => {
        unsafe {
          destroy!(device => &clear_image_memory, &clear_image, &final_buffer);
        }
        return Err(err);
      }
    };

    Ok(Self {
      clear_image,
      clear_image_memory,
      final_buffer,
      final_buffer_size: buffer_size,
      final_buffer_memory,
    })
  }

  // map can fail with vk::Result::ERROR_MEMORY_MAP_FAILED
  // in most cases it may be possible to try mapping again a smaller range
  pub unsafe fn get_buffer_data<F: FnOnce(&[u8])>(
    &self,
    device: &ash::Device,
    f: F,
  ) -> Result<(), vk::Result> {
    let ptr = device.map_memory(
      self.final_buffer_memory,
      0,
      // if size is not vk::WHOLE_SIZE, mapping should follow alignments
      vk::WHOLE_SIZE,
      vk::MemoryMapFlags::empty(),
    )? as *const u8;
    let data = std::slice::from_raw_parts(ptr, self.final_buffer_size as usize);

    f(data);

    unsafe {
      device.unmap_memory(self.final_buffer_memory);
    }

    Ok(())
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(self: &Self, device: &ash::Device) {
    self.clear_image.destroy_self(device);
    self.clear_image_memory.destroy_self(device);
    self.final_buffer.destroy_self(device);
    self.final_buffer_memory.destroy_self(device);
  }
}
