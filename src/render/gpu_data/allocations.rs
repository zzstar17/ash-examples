use std::ops::BitOr;

use ash::vk;
use vkallocator::{DetailedMemory, MappedHostBuffer};
use vkinitialization::device::{Device, PhysicalDevice};
use vkobjects::{utility::OnErr, DeviceManuallyDestroyed};

use crate::render::{
  create_objs::create_buffer,
  gpu_data::{sprite_buffers::SpriteBuffers, text::TextBuffers, GPUDataAllocationError},
};

pub fn allocate_staging_memory(
  device: &Device,
  physical_device: &PhysicalDevice,
  size: u64,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
) -> Result<MappedHostBuffer<u8>, GPUDataAllocationError> {
  let staging_buffer = create_buffer(
    device,
    size,
    vk::BufferUsageFlags::TRANSFER_SRC,
    #[cfg(feature = "vl")]
    marker,
    #[cfg(feature = "vl")]
    c"Staging buffer",
  )?;

  let (_staging_alloc, host_objects) = vkallocator::allocate_and_map_host_memory(
    device,
    physical_device,
    [
      vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_COHERENT),
      vk::MemoryPropertyFlags::HOST_VISIBLE,
    ],
    [&staging_buffer],
    0.5,
    #[cfg(feature = "log_alloc")]
    Some(["Staging buffer"]),
    #[cfg(feature = "log_alloc")]
    "Staging memory",
  )
  .on_err(|_| unsafe {
    staging_buffer.destroy_self(device);
  })?;

  Ok(host_objects[0].into_buffer())
}

pub fn allocate_device(
  device: &Device,
  physical_device: &PhysicalDevice,
  sprite_buffers: &SpriteBuffers,
  text_buffers: &TextBuffers,
  text_ui: vk::Image,
) -> Result<Vec<DetailedMemory>, GPUDataAllocationError> {
  let device_alloc = vkallocator::allocate_and_bind_memory(
    device,
    physical_device,
    [
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      vk::MemoryPropertyFlags::empty(),
    ],
    [
      &sprite_buffers.quad_vertices,
      &sprite_buffers.quad_indices,
      &sprite_buffers.texture,
      &text_ui,
      &text_buffers.curve_texture,
      &text_buffers.band_texture,
      &text_buffers.device.vertices,
      &text_buffers.device.indices,
    ],
    0.5,
    false,
    #[cfg(feature = "log_alloc")]
    Some([
      "Quad vertices",
      "Quad indices",
      "Sprite texture",
      "Text UI",
      "Text curve texture",
      "Text band texture",
      "Text device vertices",
      "Text device indices",
    ]),
    #[cfg(feature = "log_alloc")]
    "Device allocation",
  )?;

  Ok(device_alloc.get_memories().to_vec())
}

pub fn allocate_host_device(
  device: &Device,
  physical_device: &PhysicalDevice,
  text_buffers: &TextBuffers,
) -> Result<Vec<DetailedMemory>, GPUDataAllocationError> {
  let device_alloc = vkallocator::allocate_and_bind_memory(
    device,
    physical_device,
    [
      vk::MemoryPropertyFlags::DEVICE_LOCAL,
      vk::MemoryPropertyFlags::HOST_VISIBLE,
    ],
    [&text_buffers.cpu.vertices, &text_buffers.cpu.indices],
    0.5,
    false,
    #[cfg(feature = "log_alloc")]
    Some(["Text host vertices", "Text host indices"]),
    #[cfg(feature = "log_alloc")]
    "Host device data",
  )
  .or_else(|_| {
    vkallocator::allocate_and_bind_memory(
      device,
      physical_device,
      [vk::MemoryPropertyFlags::HOST_VISIBLE],
      [&text_buffers.cpu.vertices, &text_buffers.cpu.indices],
      0.5,
      false,
      #[cfg(feature = "log_alloc")]
      Some(["Text host vertices", "Text host indices"]),
      #[cfg(feature = "log_alloc")]
      "Host device data",
    )
  })?;

  Ok(device_alloc.get_memories().to_vec())
}
