use ash::vk;
use std::{
  ops::BitOr,
  ptr::{self, addr_of},
};

use crate::{
  allocator::allocate_and_bind_memory,
  command_pools::{ComputeCommandBufferPool, TransferCommandBufferPool},
  device::{create_logical_device, PhysicalDevice, Queues},
  entry,
  errors::InitializationError,
  image::create_image,
  instance::create_instance,
  IMAGE_HEIGHT, IMAGE_SAVE_TYPE, IMAGE_WIDTH,
};

fn create_semaphore(device: &ash::Device) -> vk::Semaphore {
  let create_info = vk::SemaphoreCreateInfo {
    s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::SemaphoreCreateFlags::empty(),
  };
  unsafe {
    device
      .create_semaphore(&create_info, None)
      .expect("Failed to create a semaphore")
  }
}

fn create_fence(device: &ash::Device) -> vk::Fence {
  let create_info = vk::FenceCreateInfo {
    s_type: vk::StructureType::FENCE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::FenceCreateFlags::empty(),
  };
  unsafe {
    device
      .create_fence(&create_info, None)
      .expect("Failed to create a fence")
  }
}

fn create_buffer(
  device: &ash::Device,
  size: u64,
  usage: vk::BufferUsageFlags,
) -> Result<vk::Buffer, vk::Result> {
  let create_info = vk::BufferCreateInfo {
    s_type: vk::StructureType::BUFFER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::BufferCreateFlags::empty(),
    size,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(),
  };
  unsafe { device.create_buffer(&create_info, None) }
}

pub struct Renderer {
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: crate::validation_layers::DebugUtils,
  physical_device: PhysicalDevice,
  device: ash::Device,
  queues: Queues,
  compute_pool: ComputeCommandBufferPool,
  transfer_pool: TransferCommandBufferPool,
  local_image: vk::Image,
  local_image_memory: vk::DeviceMemory,
  host_buffer: vk::Buffer,
  host_buffer_size: u64,
  host_buffer_memory: vk::DeviceMemory,
}

impl Renderer {
  pub fn initialize() -> Result<Self, InitializationError> {
    let entry: ash::Entry = unsafe { entry::get_entry() };

    #[cfg(feature = "vl")]
    let (instance, debug_utils) = create_instance(&entry)?;
    #[cfg(not(feature = "vl"))]
    let instance = instance::create_instance(&entry).map_err(to_init_error)?;

    let physical_device = match unsafe { PhysicalDevice::select(&instance) }? {
      Some(device) => device,
      None => return Err(InitializationError::NoCompatibleDevices),
    };

    let (device, queues) = create_logical_device(&instance, &physical_device)?;

    // GPU image with DEVICE_LOCAL flags
    let local_image = create_image(
      &device,
      vk::ImageUsageFlags::TRANSFER_SRC.bitor(vk::ImageUsageFlags::TRANSFER_DST),
    )?;
    log::debug!("Allocating memory for local image");
    let local_image_memory = allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      &[],
      &[],
      &[local_image],
      &[unsafe { device.get_image_memory_requirements(local_image) }],
    )
    .or_else(|err| {
      log::warn!(
        "Failed to allocate optimal memory for local image:\n{:?}",
        err
      );
      allocate_and_bind_memory(
        &device,
        &physical_device,
        vk::MemoryPropertyFlags::empty(),
        &[],
        &[],
        &[local_image],
        &[unsafe { device.get_image_memory_requirements(local_image) }],
      )
    })?
    .memory;

    let buffer_size = IMAGE_WIDTH as u64 * IMAGE_HEIGHT as u64 * 4;
    let host_buffer = create_buffer(&device, buffer_size, vk::BufferUsageFlags::TRANSFER_DST)?;
    log::debug!("Allocating memory for host buffer");
    let host_buffer_memory = allocate_and_bind_memory(
      &device,
      &physical_device,
      vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
      &[host_buffer],
      &[unsafe { device.get_buffer_memory_requirements(host_buffer) }],
      &[],
      &[],
    )
    .or_else(|err| {
      log::warn!(
        "Failed to allocate optimal memory for host buffer:\n{:?}",
        err
      );
      allocate_and_bind_memory(
        &device,
        &physical_device,
        vk::MemoryPropertyFlags::HOST_VISIBLE,
        &[host_buffer],
        &[unsafe { device.get_buffer_memory_requirements(host_buffer) }],
        &[],
        &[],
      )
    })?
    .memory;

    let compute_pool = ComputeCommandBufferPool::create(&device, &physical_device.queue_families)?;
    let transfer_pool =
      TransferCommandBufferPool::create(&device, &physical_device.queue_families)?;

    Ok(Self {
      _entry: entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      physical_device,
      device,
      queues,
      compute_pool,
      transfer_pool,
      local_image,
      local_image_memory,
      host_buffer,
      host_buffer_size: buffer_size,
      host_buffer_memory,
    })
  }

  pub unsafe fn record_work(&mut self) {
    self.compute_pool.reset(&self.device).unwrap();
    self
      .compute_pool
      .record_clear_img(
        &self.device,
        &self.physical_device.queue_families,
        self.local_image,
      )
      .unwrap();

    self.transfer_pool.reset(&self.device).unwrap();
    self
      .transfer_pool
      .record_copy_img_to_buffer(
        &self.device,
        &self.physical_device.queue_families,
        self.local_image,
        self.host_buffer,
      )
      .unwrap();
  }

  pub fn submit_and_wait(&self) {
    let image_clear_finished = create_semaphore(&self.device);
    let clear_image_submit = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: addr_of!(self.compute_pool.clear_img),
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
      p_command_buffers: addr_of!(self.transfer_pool.copy_to_host),
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
    };

    let finished = create_fence(&self.device);

    unsafe {
      // note: you can make multiple submits with device.queue_submit2
      self
        .device
        .queue_submit(
          self.queues.compute,
          &[clear_image_submit],
          vk::Fence::null(),
        )
        .expect("Failed to submit compute");
      self
        .device
        .queue_submit(self.queues.transfer, &[transfer_image_submit], finished)
        .expect("Failed to submit transfer");

      self
        .device
        .wait_for_fences(&[finished], true, u64::MAX)
        .expect("Failed to wait for fences");
    }

    unsafe {
      self.device.destroy_fence(finished, None);
      self.device.destroy_semaphore(image_clear_finished, None);
    }
  }

  pub fn save_buffer_to_image_file<P>(&self, path: P)
  where
    P: AsRef<std::path::Path>,
  {
    // image memory needs to not be busy (getting used by device)
    let image_bytes = unsafe {
      let ptr = self
        .device
        .map_memory(
          self.host_buffer_memory,
          0,
          // if size is not vk::WHOLE_SIZE, mapping should follow alignments
          vk::WHOLE_SIZE,
          vk::MemoryMapFlags::empty(),
        )
        .expect("Failed to map map memory while saving resulting buffer")
        as *const u8;
      std::slice::from_raw_parts(ptr, self.host_buffer_size as usize)
    };

    // read bytes and save to file
    image::save_buffer(
      path,
      image_bytes,
      IMAGE_WIDTH,
      IMAGE_HEIGHT,
      IMAGE_SAVE_TYPE,
    )
    .expect("Failed to save image");

    unsafe {
      self.device.unmap_memory(self.host_buffer_memory);
    }
  }
}

impl Drop for Renderer {
  fn drop(&mut self) {
    unsafe {
      // wait until all operations have finished and the device is safe to destroy
      self
        .device
        .device_wait_idle()
        .expect("Failed to wait for the device to become idle");

      self.compute_pool.destroy_self(&self.device);
      self.transfer_pool.destroy_self(&self.device);

      self.device.destroy_image(self.local_image, None);
      self.device.free_memory(self.local_image_memory, None);
      self.device.destroy_buffer(self.host_buffer, None);
      self.device.free_memory(self.host_buffer_memory, None);

      log::debug!("Destroying device");
      self.device.destroy_device(None);

      #[cfg(feature = "vl")]
      {
        self.debug_utils.destroy_self();
      }
      self.instance.destroy_instance(None);
    }
  }
}
