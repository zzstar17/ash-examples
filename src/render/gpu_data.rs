use std::ops::BitOr;

use crate::render::{
  command_pools::{self, initialization::PendingInitialization},
  create_objs::{create_buffer, create_image, create_image_view},
  render_object::{QUAD_INDICES, QUAD_INDICES_SIZE, VERTICES, VERTICES_SIZE},
};
use ash::vk;
use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};
use vkobjects::{
  const_flag_bitor, destroy, errors::QueueSubmitError, utility::OnErr, DeviceManuallyDestroyed,
};

use vkallocator::{DeviceMemoryInitializationError, MemoryWithType, SingleUseStagingBuffers};

pub const TEXTURE_USAGES: vk::ImageUsageFlags = const_flag_bitor!(
  vk::ImageUsageFlags =>
  vk::ImageUsageFlags::SAMPLED,
  vk::ImageUsageFlags::TRANSFER_DST
);
pub const TEXTURE_FORMAT_FEATURES: vk::FormatFeatureFlags = const_flag_bitor!(
  vk::FormatFeatureFlags =>
  vk::FormatFeatureFlags::TRANSFER_DST,
  vk::FormatFeatureFlags::SAMPLED_IMAGE
);

#[derive(Debug)]
pub struct GPUData {
  pub texture: vk::Image,
  pub texture_view: vk::ImageView,

  pub vertex_buffer: vk::Buffer,
  pub index_buffer: vk::Buffer,
  // pub instance_buffers: [vk::Buffer; FRAMES_IN_FLIGHT],
  memories: Vec<MemoryWithType>,
}

#[must_use]
#[derive(Debug)]
pub struct PendingDataInitialization {
  command_buffer_submit: PendingInitialization,
  staging_buffers: SingleUseStagingBuffers<3>,
}

impl PendingDataInitialization {
  // should not fail
  pub unsafe fn wait_and_self_destroy(&self, device: &ash::Device) -> Result<(), QueueSubmitError> {
    self.command_buffer_submit.wait_and_self_destroy(device)?;
    self.staging_buffers.destroy_self(device);
    Ok(())
  }
}

impl DeviceManuallyDestroyed for PendingDataInitialization {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    log::warn!("Aborting and destroying PendingDataInitialization");
    if let Err(err) = self.wait_and_self_destroy(device) {
      log::error!("PendingDataInitialization failed to destroy self: {}", err);
    }
  }
}

fn create_and_copy_from_staging_buffers(
  device: &Device,
  physical_device: &PhysicalDevice,
  queues: &SingleQueues,
  vertex_buffer: vk::Buffer,
  index_buffer: vk::Buffer,
  texture: vk::Image,
  texture_extent: vk::Extent2D,
  texture_data: Vec<u8>,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
) -> Result<PendingDataInitialization, DeviceMemoryInitializationError> {
  let graphics_pool = command_pools::initialization::InitCommandBufferPool::new(
    device,
    physical_device.queue_families.graphics.index,
    #[cfg(feature = "vl")]
    marker,
  )?;
  unsafe {
    let staging_buffers = vkallocator::create_single_use_staging_buffers(
      device,
      physical_device,
      [
        (texture_data.as_ptr(), texture_data.len() as u64),
        (VERTICES.as_ptr() as *const u8, VERTICES_SIZE),
        (QUAD_INDICES.as_ptr() as *const u8, QUAD_INDICES_SIZE),
      ],
      #[cfg(feature = "log_alloc")]
      "Staging buffers",
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| graphics_pool.destroy_self(device))?;

    graphics_pool.record_copy_staging_buffer_to_image(
      device,
      staging_buffers.buffers[0],
      texture,
      texture_extent,
      vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[1],
      vertex_buffer,
      VERTICES_SIZE,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[2],
      index_buffer,
      QUAD_INDICES_SIZE,
    );

    let submit = graphics_pool
      .end_and_submit(
        device,
        queues.graphics.handle,
        #[cfg(feature = "vl")]
        marker,
      )
      .on_err(|(pool, _err)| destroy!(device => &staging_buffers, pool))
      .map_err(|(_, err)| err)?;

    Ok(PendingDataInitialization {
      command_buffer_submit: submit,
      staging_buffers,
    })
  }
}

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    texture_extent: vk::Extent2D,
    texture_format: vk::Format,
    texture_data: Vec<u8>,
    queues: &SingleQueues,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<(Self, PendingDataInitialization), DeviceMemoryInitializationError> {
    let texture = create_image(
      device,
      texture_format,
      texture_extent.width,
      texture_extent.height,
      TEXTURE_USAGES,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Texture",
    )?;
    let vertex_buffer: vk::Buffer = create_buffer(
      device,
      VERTICES_SIZE,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Vertex buffer",
    )
    .on_err(|_| unsafe { texture.destroy_self(device) })?;
    let index_buffer: vk::Buffer = create_buffer(
      device,
      QUAD_INDICES_SIZE,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Index buffer",
    )
    .on_err(|_| unsafe { destroy!(device => &vertex_buffer, &texture) })?;

    let device_alloc = vkallocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
      ],
      [&texture, &vertex_buffer, &index_buffer],
      0.5,
      #[cfg(feature = "log_alloc")]
      Some(["Target image", "Vertex buffer", "Index buffer"]),
      #[cfg(feature = "log_alloc")]
      "Constant data",
    )
    .on_err(|_| unsafe { destroy!(device => &texture, &index_buffer, &vertex_buffer) })?;

    let pending_device_init = create_and_copy_from_staging_buffers(
      device,
      physical_device,
      queues,
      vertex_buffer,
      index_buffer,
      texture,
      texture_extent,
      texture_data,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe {
      destroy!(device => &texture, &index_buffer, &vertex_buffer, &device_alloc)
    })?;

    const EXPECTED_MAX_MEM_COUNT: usize = 3;
    let mut memories = Vec::with_capacity(EXPECTED_MAX_MEM_COUNT);
    memories.extend_from_slice(device_alloc.get_memories());
    memories.shrink_to_fit();

    debug_assert!(
      memories.len() <= EXPECTED_MAX_MEM_COUNT,
      "Allocating more than expected"
    );
    log::info!("Allocated memory count: {}", memories.len());

    let texture_view = create_image_view(device, texture, texture_format)
    .on_err(|_| unsafe {destroy!(device => &pending_device_init, &texture, &index_buffer, &vertex_buffer, memories.as_slice()) })?;

    Ok((
      Self {
        texture,
        texture_view,
        vertex_buffer,
        index_buffer,
        memories,
      },
      pending_device_init,
    ))
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.texture_view.destroy_self(device);
    self.texture.destroy_self(device);

    self.vertex_buffer.destroy_self(device);
    self.index_buffer.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
