use std::ops::BitOr;

use crate::{
  render::{
    command_pools::{self, initialization::PendingInitialization},
    create_objs::{create_buffer, create_image, create_image_view},
    render_object::{QUAD_INDICES, QUAD_INDICES_SIZE, VERTICES, VERTICES_SIZE},
  },
  slug::{self, PrepareTextResult, SlugVertex},
};
use ash::vk;
use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};
use vkobjects::{
  const_flag_bitor, destroy,
  errors::{OutOfMemoryError, QueueSubmitError},
  utility::OnErr,
  DeviceManuallyDestroyed,
};

use vkallocator::{
  AllocationError, DetailedMemory, DeviceMemoryInitializationError, HostAllocationError,
  SingleUseStagingBuffers,
};

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

#[derive(Debug, thiserror::Error)]
pub enum GPUDataAllocationError {
  #[error(transparent)]
  StagingBufferError(#[from] DeviceMemoryInitializationError),
  #[error("Failed to allocate one of the main device memory objects.\n{0}")]
  AllocationError(#[from] AllocationError),
  #[error("Failed to allocate one of the main host memory objects.\n{0}")]
  HostAllocationError(#[from] HostAllocationError),
  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),
  #[error("Failed to submit allocation workload to a queue: {0}")]
  QueueSubmitError(#[from] QueueSubmitError),
}

#[derive(Debug)]
pub struct GPUData {
  pub texture: vk::Image,
  pub texture_view: vk::ImageView,

  pub vertex_buffer: vk::Buffer,
  pub index_buffer: vk::Buffer,

  pub text_curve_texture: vk::Image,
  pub text_curve_texture_view: vk::ImageView,
  pub text_band_texture: vk::Image,
  pub text_band_texture_view: vk::ImageView,

  pub text_vertices: vk::Buffer,
  pub text_indices: vk::Buffer,
  pub text_index_count: u32,

  memories: Vec<DetailedMemory>,
}

#[must_use]
#[derive(Debug)]
pub struct PendingDataInitialization {
  command_buffer_submit: PendingInitialization,
  staging_buffers: SingleUseStagingBuffers<7>,
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
  text_data: &PrepareTextResult,
  text_curve_texture: vk::Image,
  text_band_texture: vk::Image,
  text_vertices_buffer: vk::Buffer,
  text_indices_buffer: vk::Buffer,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
) -> Result<PendingDataInitialization, GPUDataAllocationError> {
  let text_vertices_size = (text_data.vertices.len() * size_of::<SlugVertex>()) as u64;
  let text_indices_size = (text_data.indices.len() * size_of::<u32>()) as u64;
  let text_curve_texture_size = (text_data.curve_tex_data.len() * size_of::<[f32; 4]>()) as u64;
  let text_band_texture_size = (text_data.band_tex_data.len() * size_of::<u32>()) as u64;
  let text_curve_extent = vk::Extent2D {
    width: slug::TEX_WIDTH as u32,
    height: text_data.curve_tex_height as u32,
  };
  let text_band_extent = vk::Extent2D {
    width: slug::TEX_WIDTH as u32,
    height: text_data.band_tex_height as u32,
  };

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
        (
          text_data.curve_tex_data.as_ptr() as *const u8,
          text_curve_texture_size,
        ),
        (
          text_data.band_tex_data.as_ptr() as *const u8,
          text_band_texture_size,
        ),
        (VERTICES.as_ptr() as *const u8, VERTICES_SIZE),
        (QUAD_INDICES.as_ptr() as *const u8, QUAD_INDICES_SIZE),
        (text_data.vertices.as_ptr() as *const u8, text_vertices_size),
        (text_data.indices.as_ptr() as *const u8, text_indices_size),
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
    graphics_pool.record_copy_staging_buffer_to_image(
      device,
      staging_buffers.buffers[1],
      text_curve_texture,
      text_curve_extent,
      vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
    graphics_pool.record_copy_staging_buffer_to_image(
      device,
      staging_buffers.buffers[2],
      text_band_texture,
      text_band_extent,
      vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[3],
      vertex_buffer,
      VERTICES_SIZE,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[4],
      index_buffer,
      QUAD_INDICES_SIZE,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[5],
      text_vertices_buffer,
      text_vertices_size,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[6],
      text_indices_buffer,
      text_indices_size,
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
    text_data: &PrepareTextResult,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<(Self, PendingDataInitialization), GPUDataAllocationError> {
    let text_vertices_size = (text_data.vertices.len() * size_of::<SlugVertex>()) as u64;
    let text_indices_size = (text_data.indices.len() * size_of::<u32>()) as u64;

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
    // todo: add device checking for this
    // todo: add in function cleanup
    let text_curve_texture = create_image(
      device,
      vk::Format::R32G32B32A32_SFLOAT,
      slug::TEX_WIDTH as u32,
      text_data.curve_tex_height as u32,
      TEXTURE_USAGES,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text curve texture",
    )
    .on_err(|_| unsafe { texture.destroy_self(device) })?;
    let text_band_texture = create_image(
      device,
      vk::Format::R32G32B32A32_UINT,
      slug::TEX_WIDTH as u32,
      text_data.band_tex_height as u32,
      TEXTURE_USAGES,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text band texture",
    )
    .on_err(|_| unsafe { texture.destroy_self(device) })?;

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

    let text_vertices_buffer: vk::Buffer = create_buffer(
      device,
      text_vertices_size,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text vertices",
    )
    .on_err(|_| unsafe { destroy!(device => &index_buffer, &vertex_buffer, &texture) })?;
    let text_indices_buffer: vk::Buffer = create_buffer(
      device,
      text_indices_size,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text indices",
    )
    .on_err(|_| unsafe {
      destroy!(device => &text_vertices_buffer, &index_buffer, &vertex_buffer, &texture)
    })?;

    let device_alloc = vkallocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
      ],
      [&texture, &text_curve_texture, &text_band_texture, &vertex_buffer, &index_buffer, &text_vertices_buffer, &text_indices_buffer],
      0.5,
      false,
      #[cfg(feature = "log_alloc")]
      Some(["Ferris", "Text curve texture", "Text band texture", "Vertex buffer", "Index buffer", "Text vertices", "Text indices"]),
      #[cfg(feature = "log_alloc")]
      "Constant data",
    )
    .on_err(|_| unsafe { destroy!(device => &text_indices_buffer, &text_vertices_buffer, &texture, &index_buffer, &vertex_buffer) })?;

    let pending_device_init = create_and_copy_from_staging_buffers(
      device,
      physical_device,
      queues,
      vertex_buffer,
      index_buffer,
      texture,
      texture_extent,
      texture_data,
      text_data,
      text_curve_texture,
      text_band_texture,
      text_vertices_buffer,
      text_indices_buffer,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe {
      destroy!(device =>  &text_indices_buffer, &text_vertices_buffer, &texture, &index_buffer, &vertex_buffer, &device_alloc)
    })?;

    const EXPECTED_MAX_MEM_COUNT: usize = 5;
    let mut memories = Vec::with_capacity(EXPECTED_MAX_MEM_COUNT);
    memories.extend_from_slice(device_alloc.get_memories());
    memories.shrink_to_fit();

    debug_assert!(
      memories.len() <= EXPECTED_MAX_MEM_COUNT,
      "Allocating more than expected"
    );
    log::info!("Allocated memory count: {}", memories.len());

    let texture_view = create_image_view(device, texture, texture_format)
    .on_err(|_| unsafe {destroy!(device => &pending_device_init, &text_indices_buffer, &text_vertices_buffer, &texture, &index_buffer, &vertex_buffer, memories.as_slice()) })?;

    // todo: error handling, variable for format
    let text_curve_texture_view =
      create_image_view(device, text_curve_texture, vk::Format::R32G32B32A32_SFLOAT)?;
    let text_band_texture_view =
      create_image_view(device, text_band_texture, vk::Format::R32G32B32A32_UINT)?;

    Ok((
      Self {
        texture,
        texture_view,
        vertex_buffer,
        index_buffer,
        memories,
        text_vertices: text_vertices_buffer,
        text_indices: text_indices_buffer,
        text_band_texture,
        text_curve_texture,
        text_curve_texture_view,
        text_band_texture_view,
        text_index_count: text_data.indices.len() as u32,
      },
      pending_device_init,
    ))
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.texture_view.destroy_self(device);
    self.texture.destroy_self(device);
    self.text_curve_texture_view.destroy_self(device);
    self.text_band_texture_view.destroy_self(device);
    self.text_curve_texture.destroy_self(device);
    self.text_band_texture.destroy_self(device);

    self.vertex_buffer.destroy_self(device);
    self.index_buffer.destroy_self(device);

    self.text_vertices.destroy_self(device);
    self.text_indices.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
