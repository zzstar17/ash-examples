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
pub struct TextBuffers {
  pub curve_texture: vk::Image,
  pub band_texture: vk::Image,

  pub vertices: vk::Buffer,
  pub indices: vk::Buffer,
  pub index_count: u32,
}

#[derive(Debug)]
pub struct GPUData {
  pub texture: vk::Image,
  pub texture_view: vk::ImageView,

  pub vertex_buffer: vk::Buffer,
  pub index_buffer: vk::Buffer,

  pub text: TextBuffers,
  pub text_curve_view: vk::ImageView,
  pub text_band_view: vk::ImageView,

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

impl TextBuffers {
  const CURVES_FORMAT: vk::Format = vk::Format::R32G32B32A32_SFLOAT;
  const BANDS_FORMAT: vk::Format = vk::Format::R32G32B32A32_UINT;

  pub fn new(
    device: &Device,
    text_data: &PrepareTextResult,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let text_vertices_size = (text_data.vertices.len() * size_of::<SlugVertex>()) as u64;
    let text_indices_size = (text_data.indices.len() * size_of::<u32>()) as u64;

    // todo: add device checking for this
    let curve_texture = create_image(
      device,
      Self::CURVES_FORMAT,
      slug::TEX_WIDTH as u32,
      text_data.curve_tex_height as u32,
      TEXTURE_USAGES,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text curve texture",
    )?;
    let band_texture = create_image(
      device,
      Self::BANDS_FORMAT,
      slug::TEX_WIDTH as u32,
      text_data.band_tex_height as u32,
      TEXTURE_USAGES,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text band texture",
    )
    .on_err(|_| unsafe { curve_texture.destroy_self(device) })?;

    let vertices: vk::Buffer = create_buffer(
      device,
      text_vertices_size,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text vertices",
    )
    .on_err(|_| unsafe { destroy!(device => &band_texture, &curve_texture) })?;
    let indices: vk::Buffer = create_buffer(
      device,
      text_indices_size,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text indices",
    )
    .on_err(|_| unsafe { destroy!(device => &vertices, &band_texture, &curve_texture) })?;

    Ok(Self {
      curve_texture,
      band_texture,
      vertices,
      indices,
      index_count: text_data.indices.len() as u32,
    })
  }
}

impl DeviceManuallyDestroyed for TextBuffers {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.curve_texture.destroy_self(device);
    self.band_texture.destroy_self(device);

    self.vertices.destroy_self(device);
    self.indices.destroy_self(device);
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
  text_buffers: &TextBuffers,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
) -> Result<PendingDataInitialization, GPUDataAllocationError> {
  let text_vertices_size = (text_data.vertices.len() * size_of::<SlugVertex>()) as u64;
  let text_indices_size = (text_data.indices.len() * size_of::<u32>()) as u64;

  let text_curve_len = slug::TEX_WIDTH * text_data.curve_tex_height * 4;
  let text_band_len = slug::TEX_WIDTH * text_data.band_tex_height * 4;

  let text_curve_texture_size = (text_curve_len * size_of::<f32>()) as u64;
  let text_band_texture_size = (text_band_len * size_of::<u32>()) as u64;
  let text_curve_extent = vk::Extent2D {
    width: slug::TEX_WIDTH as u32,
    height: text_data.curve_tex_height as u32,
  };
  let text_band_extent = vk::Extent2D {
    width: slug::TEX_WIDTH as u32,
    height: text_data.band_tex_height as u32,
  };
  assert_eq!(text_data.curve_tex_data.len() * 4, text_curve_len);
  assert_eq!(text_data.band_tex_data.len(), text_band_len);

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

    // todo: some synch error makes text sometimes not appear
    // validation layers show nothing, maybe data is not being flushed properly?
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
      text_buffers.curve_texture,
      text_curve_extent,
      vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    );
    graphics_pool.record_copy_staging_buffer_to_image(
      device,
      staging_buffers.buffers[2],
      text_buffers.band_texture,
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
      text_buffers.vertices,
      text_vertices_size,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[6],
      text_buffers.indices,
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

    let text_buffers = TextBuffers::new(
      device,
      text_data,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { destroy!(device => &index_buffer, &vertex_buffer, &texture) })?;

    let device_alloc = vkallocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
      ],
      [
        &texture,
        &text_buffers.curve_texture,
        &text_buffers.band_texture,
        &vertex_buffer,
        &index_buffer,
        &text_buffers.vertices,
        &text_buffers.indices,
      ],
      0.5,
      false,
      #[cfg(feature = "log_alloc")]
      Some([
        "Ferris",
        "Text curve texture",
        "Text band texture",
        "Vertex buffer",
        "Index buffer",
        "Text vertices",
        "Text indices",
      ]),
      #[cfg(feature = "log_alloc")]
      "Constant data",
    )
    .on_err(|_| unsafe {
      destroy!(device => &text_buffers, &texture, &index_buffer, &vertex_buffer)
    })?;

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
      &text_buffers,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe {
      destroy!(device =>  &text_buffers, &texture, &index_buffer, &vertex_buffer, &device_alloc)
    })?;

    let memories = device_alloc.get_memories().to_vec();
    log::info!("Allocated memory count: {}", memories.len());

    let texture_view = create_image_view(device, texture, texture_format)
    .on_err(|_| unsafe {destroy!(device => &pending_device_init, &text_buffers, &texture, &index_buffer, &vertex_buffer, memories.as_slice()) })?;

    let text_curve_view = create_image_view(device, text_buffers.curve_texture, TextBuffers::CURVES_FORMAT).on_err(|_| unsafe {destroy!(device => &texture_view, &pending_device_init, &text_buffers, &texture, &index_buffer, &vertex_buffer, memories.as_slice()) })?;
    let text_band_view = create_image_view(device, text_buffers.band_texture, TextBuffers::BANDS_FORMAT).on_err(|_| unsafe {destroy!(device => &text_curve_view, &texture_view, &pending_device_init, &text_buffers, &texture, &index_buffer, &vertex_buffer, memories.as_slice()) })?;

    Ok((
      Self {
        texture,
        texture_view,
        vertex_buffer,
        index_buffer,
        memories,
        text: text_buffers,
        text_band_view,
        text_curve_view,
      },
      pending_device_init,
    ))
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.texture_view.destroy_self(device);
    self.text_curve_view.destroy_self(device);
    self.text_band_view.destroy_self(device);

    self.texture.destroy_self(device);

    self.vertex_buffer.destroy_self(device);
    self.index_buffer.destroy_self(device);

    self.text.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
