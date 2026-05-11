use std::{marker::PhantomData, ptr};

use ash::vk;

mod graphics;
pub mod initialization;
mod transfer;

pub use graphics::GraphicsCommandBufferPool;
pub use transfer::TransferCommandBufferPool;
use vkinitialization::device::PhysicalDevice;
use vkobjects::{errors::OutOfMemoryError, utility::OnErr, DeviceManuallyDestroyed};

fn create_command_pool(
  device: &ash::Device,
  flags: vk::CommandPoolCreateFlags,
  queue_family_index: u32,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  #[cfg(feature = "vl")] name: &std::ffi::CStr,
) -> Result<vk::CommandPool, OutOfMemoryError> {
  let command_pool_create_info = vk::CommandPoolCreateInfo {
    s_type: vk::StructureType::COMMAND_POOL_CREATE_INFO,
    p_next: ptr::null(),
    flags,
    queue_family_index,
    _marker: PhantomData,
  };
  unsafe {
    let command_pool = device.create_command_pool(&command_pool_create_info, None)?;
    #[cfg(feature = "vl")]
    marker.set_obj_name(
      vk::ObjectType::COMMAND_POOL,
      vk::Handle::as_raw(command_pool),
      name,
    )?;
    Ok(command_pool)
  }
}

fn allocate_primary_command_buffers(
  device: &ash::Device,
  command_pool: vk::CommandPool,
  command_buffer_count: u32,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  #[cfg(feature = "vl")] names: &[&std::ffi::CStr],
) -> Result<Vec<vk::CommandBuffer>, OutOfMemoryError> {
  let allocate_info = vk::CommandBufferAllocateInfo {
    s_type: vk::StructureType::COMMAND_BUFFER_ALLOCATE_INFO,
    p_next: ptr::null(),
    command_buffer_count,
    command_pool,
    level: vk::CommandBufferLevel::PRIMARY,
    _marker: PhantomData,
  };

  unsafe {
    let buffers = device.allocate_command_buffers(&allocate_info)?;
    #[cfg(feature = "vl")]
    {
      for (&buffer, &name) in buffers.iter().zip(names.iter()) {
        marker.set_obj_name(
          vk::ObjectType::COMMAND_BUFFER,
          vk::Handle::as_raw(buffer),
          name,
        )?;
      }
    }
    Ok(buffers)
  }
}

fn dependency_info<'a>(
  memory: &'a [vk::MemoryBarrier2],
  buffer: &'a [vk::BufferMemoryBarrier2],
  image: &'a [vk::ImageMemoryBarrier2],
) -> vk::DependencyInfo<'a> {
  vk::DependencyInfo {
    s_type: vk::StructureType::DEPENDENCY_INFO,
    p_next: ptr::null(),
    dependency_flags: vk::DependencyFlags::empty(),
    memory_barrier_count: memory.len() as u32,
    p_memory_barriers: memory.as_ptr(),
    buffer_memory_barrier_count: buffer.len() as u32,
    p_buffer_memory_barriers: buffer.as_ptr(),
    image_memory_barrier_count: image.len() as u32,
    p_image_memory_barriers: image.as_ptr(),
    _marker: PhantomData,
  }
}

pub struct CommandPools {
  pub graphics_pool: GraphicsCommandBufferPool,
  pub transfer_pool: TransferCommandBufferPool,
}

impl CommandPools {
  pub fn new(
    device: &ash::Device,
    physical_device: &PhysicalDevice,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, OutOfMemoryError> {
    let graphics_pool = GraphicsCommandBufferPool::create(
      device,
      &physical_device.queue_families,
      #[cfg(feature = "vl")]
      marker,
    )?;
    let transfer_pool = TransferCommandBufferPool::create(
      device,
      &physical_device.queue_families,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { graphics_pool.destroy_self(device) })?;
    Ok(Self {
      graphics_pool,
      transfer_pool,
    })
  }
}

impl DeviceManuallyDestroyed for CommandPools {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.graphics_pool.destroy_self(device);
    self.transfer_pool.destroy_self(device);
  }
}
