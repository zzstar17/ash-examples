use ash::vk;
use ash_destructor::{DeviceDestroyable, SelfDestroyable};
use std::{marker::PhantomData, ops::BitOr, ptr};

use crate::{
  allocator::{self, MemoryWithType},
  command_pools::CommandPools,
  create_objs::{create_buffer, create_fence, create_image, create_semaphore},
  device::{Device, PhysicalDevice, Queues},
  device_destroyable::destroy,
  entry,
  errors::{InitializationError, OutOfMemoryError},
  instance::create_instance,
  utility::OnErr,
};

#[derive(DeviceDestroyable)]
pub struct Renderer {
  #[destroy_ignore]
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: crate::validation_layers::DebugUtils,
  #[destroy_ignore]
  physical_device: PhysicalDevice,
  device: Device,
  #[destroy_ignore]
  queues: Queues,
  command_pools: CommandPools,
  gpu_data: GPUData,
}

#[derive(DeviceDestroyable)]
struct GPUData {
  clear_image_memory: vk::DeviceMemory,
  clear_image: vk::Image,

  final_buffer_memory: MemoryWithType,
  final_buffer: vk::Buffer,
  #[destroy_ignore]
  final_buffer_size: u64,
}

impl SelfDestroyable for Renderer {
  unsafe fn destroy_self_alloc(&self, allocation_callbacks: Option<&vk::AllocationCallbacks>) {
    DeviceDestroyable::destroy_self_alloc(&self, &self.device, allocation_callbacks);
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

      SelfDestroyable::destroy_self(self);
    }
  }
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

    let destroy_instance = || unsafe {
      #[cfg(feature = "vl")]
      destroy!(&debug_utils);
      destroy!(&instance);
    };

    let physical_device =
      match unsafe { PhysicalDevice::select(&instance) }.on_err(|_| destroy_instance())? {
        Some(device) => device,
        None => {
          destroy_instance();
          return Err(InitializationError::NoCompatibleDevices);
        }
      };

    let (device, queues) =
      Device::create(&instance, &physical_device).on_err(|_| destroy_instance())?;

    let command_pools = CommandPools::new(&device, &physical_device).on_err(|_| unsafe {
      destroy!(&device);
      destroy_instance();
    })?;

    let gpu_data = GPUData::new(
      &device,
      &physical_device,
      image_width,
      image_height,
      buffer_size,
    )
    .on_err(|_| unsafe {
      destroy!(&device => &command_pools, &device);
      destroy_instance();
    })?;

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
      p_command_buffers: &self.command_pools.compute_pool.clear_img,
      signal_semaphore_count: 1,
      p_signal_semaphores: &image_clear_finished,
      _marker: PhantomData,
    };
    let wait_for = vk::PipelineStageFlags::TRANSFER;
    let transfer_image_submit = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 1,
      p_wait_semaphores: &image_clear_finished,
      p_wait_dst_stage_mask: &wait_for,
      command_buffer_count: 1,
      p_command_buffers: &self.command_pools.transfer_pool.copy_image_to_buffer,
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
      _marker: PhantomData,
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

  pub unsafe fn get_resulting_data(&self) -> Result<&[u8], vk::Result> {
    self
      .gpu_data
      .map_buffer_after_completion(&self.device, &self.physical_device)
  }
}

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    image_width: u32,
    image_height: u32,
    buffer_size: u64,
  ) -> Result<Self, allocator::AllocationError> {
    // GPU image with DEVICE_LOCAL flags
    let clear_image = create_image(
      device,
      image_width,
      image_height,
      vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::TRANSFER_DST),
    )?;
    let clear_image_alloc = allocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
      ],
      [&clear_image],
      0.5,
      #[cfg(feature = "log_alloc")]
      Some(["Image that is cleared on the compute queue"]),
      #[cfg(feature = "log_alloc")]
      "PROGRAM'S CLEAR IMAGE",
    )
    .on_err(|_| unsafe {
      clear_image.destroy_self(device);
    })
    .on_err(|_| unsafe { clear_image.destroy_self(device) })?;
    let clear_image_memory = clear_image_alloc.get_memories()[0].memory;

    let final_buffer = create_buffer(device, buffer_size, vk::BufferUsageFlags::TRANSFER_DST)
      .on_err(|_| unsafe {
        destroy!(device => &clear_image_memory, &clear_image);
      })?;
    let final_buffer_alloc = allocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
        vk::MemoryPropertyFlags::HOST_VISIBLE,
      ],
      [&final_buffer],
      0.5,
      #[cfg(feature = "log_alloc")]
      Some(["Buffer where the final data is read from"]),
      #[cfg(feature = "log_alloc")]
      "OUTPUT BUFFER",
    )
    .on_err(|_| unsafe {
      destroy!(device => &clear_image_memory, &clear_image, &final_buffer);
    })?;
    let final_buffer_memory = final_buffer_alloc.get_memories()[0];

    Ok(Self {
      clear_image,
      clear_image_memory,
      final_buffer,
      final_buffer_size: buffer_size,
      final_buffer_memory: final_buffer_memory,
    })
  }

  // returns a slice representing buffer contents after all operations have completed
  // map can fail with vk::Result::ERROR_MEMORY_MAP_FAILED
  // in most cases it may be possible to try mapping again a smaller range
  pub unsafe fn map_buffer_after_completion(
    &self,
    device: &ash::Device,
    physical_device: &PhysicalDevice,
  ) -> Result<&[u8], vk::Result> {
    if !physical_device.mem_properties.memory_types[self.final_buffer_memory.type_index]
      .property_flags
      .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
    {
      let range = vk::MappedMemoryRange {
        s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
        p_next: ptr::null(),
        memory: *self.final_buffer_memory,
        offset: 0,
        size: vk::WHOLE_SIZE,
        _marker: PhantomData,
      };
      device.invalidate_mapped_memory_ranges(&[range])?;
    }

    let ptr = device.map_memory(
      *self.final_buffer_memory,
      0,
      // if size is not vk::WHOLE_SIZE, mapping should follow alignments
      vk::WHOLE_SIZE,
      vk::MemoryMapFlags::empty(),
    )? as *const u8;

    Ok(std::slice::from_raw_parts(
      ptr,
      self.final_buffer_size as usize,
    ))
  }
}
