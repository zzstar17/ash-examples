use std::ops::BitOr;

use crate::{
  render::{
    command_pools::{self, initialization::PendingInitialization},
    create_objs::{create_buffer, create_image, create_image_view},
    render_object::{QUAD_INDICES, QUAD_INDICES_SIZE, VERTICES, VERTICES_SIZE},
  },
  slug::{self, MultilineRect, SlugTextureData, SlugVertex},
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

#[derive(Debug, Clone, Copy)]
pub struct SpriteBuffers {
  pub texture: vk::Image,

  pub quad_vertices: vk::Buffer,
  pub quad_indices: vk::Buffer,
}

#[derive(Debug, Clone, Copy)]
pub struct TextBuffers {
  pub curve_texture: vk::Image,
  pub band_texture: vk::Image,

  pub vertices: vk::Buffer,
  pub indices: vk::Buffer,
  pub index_count: u32,
}

pub struct TextData<'a> {
  pub textures: SlugTextureData<'a>,
  pub vertices: &'a [SlugVertex],
  pub indices: &'a [u32],
  pub size: MultilineRect,
}

pub struct ImageViews {
  pub sprite: vk::ImageView,
  pub text_curve: vk::ImageView,
  pub text_band: vk::ImageView,
  pub ui: vk::ImageView,
}

#[derive(Debug)]
pub struct GPUData {
  pub sprite_buffers: SpriteBuffers,
  pub sprite_view: vk::ImageView,

  pub text: TextBuffers,
  pub text_curve_view: vk::ImageView,
  pub text_band_view: vk::ImageView,
  pub text_size: MultilineRect,

  pub ui: vk::Image,
  pub ui_view: vk::ImageView,
  pub ui_size: vk::Extent2D,

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

impl SpriteBuffers {
  pub fn new(
    device: &Device,
    texture_extent: vk::Extent2D,
    render_format: vk::Format,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let texture = create_image(
      device,
      render_format,
      texture_extent.width,
      texture_extent.height,
      TEXTURE_USAGES,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Texture",
    )?;

    let quad_vertices: vk::Buffer = create_buffer(
      device,
      VERTICES_SIZE,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Vertex buffer",
    )
    .on_err(|_| unsafe { texture.destroy_self(device) })?;
    let quad_indices: vk::Buffer = create_buffer(
      device,
      QUAD_INDICES_SIZE,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Index buffer",
    )
    .on_err(|_| unsafe { destroy!(device => &quad_vertices, &texture) })?;

    Ok(Self {
      texture,
      quad_vertices,
      quad_indices,
    })
  }
}

impl TextBuffers {
  const CURVES_FORMAT: vk::Format = vk::Format::R32G32B32A32_SFLOAT;
  const BANDS_FORMAT: vk::Format = vk::Format::R32G32B32A32_UINT;

  pub fn new(
    device: &Device,
    text_data: &TextData,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let text_vertices_size = (text_data.vertices.len() * size_of::<SlugVertex>()) as u64;
    let text_indices_size = (text_data.indices.len() * size_of::<u32>()) as u64;

    // todo: add device checking for this
    let curve_texture = create_image(
      device,
      Self::CURVES_FORMAT,
      slug::TEX_WIDTH as u32,
      text_data.textures.curve_tex_height as u32,
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
      text_data.textures.band_tex_height as u32,
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

impl ImageViews {
  pub fn new(
    device: &Device,
    render_format: vk::Format,
    sprite_buffers: &SpriteBuffers,
    text_buffers: &TextBuffers,
    ui: vk::Image,
  ) -> Result<Self, OutOfMemoryError> {
    let sprite = create_image_view(device, sprite_buffers.texture, render_format)?;

    let text_curve = create_image_view(
      device,
      text_buffers.curve_texture,
      TextBuffers::CURVES_FORMAT,
    )
    .on_err(|_| unsafe { destroy!(device => &sprite) })?;
    let text_band = create_image_view(device, text_buffers.band_texture, TextBuffers::BANDS_FORMAT)
      .on_err(|_| unsafe { destroy!(device => &text_curve, &sprite) })?;

    let ui = create_image_view(device, ui, render_format)
      .on_err(|_| unsafe { destroy!(device => &text_band, &text_curve, &sprite) })?;

    Ok(Self {
      sprite,
      text_curve,
      text_band,
      ui,
    })
  }
}

fn create_and_copy_from_staging_buffers(
  device: &Device,
  physical_device: &PhysicalDevice,
  queues: &SingleQueues,
  sprite_buffers: &SpriteBuffers,
  sprite_texture_extent: vk::Extent2D,
  sprite_texture_data: Vec<u8>,
  text_data: &TextData,
  text_buffers: &TextBuffers,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
) -> Result<PendingDataInitialization, GPUDataAllocationError> {
  let text_vertices_size = (text_data.vertices.len() * size_of::<SlugVertex>()) as u64;
  let text_indices_size = (text_data.indices.len() * size_of::<u32>()) as u64;

  let text_curve_len = slug::TEX_WIDTH * text_data.textures.curve_tex_height * 4;
  let text_band_len = slug::TEX_WIDTH * text_data.textures.band_tex_height * 4;

  let text_curve_texture_size = (text_curve_len * size_of::<f32>()) as u64;
  let text_band_texture_size = (text_band_len * size_of::<u32>()) as u64;
  let text_curve_extent = vk::Extent2D {
    width: slug::TEX_WIDTH as u32,
    height: text_data.textures.curve_tex_height as u32,
  };
  let text_band_extent = vk::Extent2D {
    width: slug::TEX_WIDTH as u32,
    height: text_data.textures.band_tex_height as u32,
  };
  assert_eq!(text_data.textures.curve_tex_data.len() * 4, text_curve_len);
  assert_eq!(text_data.textures.band_tex_data.len(), text_band_len);

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
        (
          sprite_texture_data.as_ptr(),
          sprite_texture_data.len() as u64,
        ),
        (
          text_data.textures.curve_tex_data.as_ptr() as *const u8,
          text_curve_texture_size,
        ),
        (
          text_data.textures.band_tex_data.as_ptr() as *const u8,
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
      sprite_buffers.texture,
      sprite_texture_extent,
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
      sprite_buffers.quad_vertices,
      VERTICES_SIZE,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[4],
      sprite_buffers.quad_indices,
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
    render_format: vk::Format,
    sprite_texture_extent: vk::Extent2D,
    sprite_texture_data: Vec<u8>,
    queues: &SingleQueues,
    text_data: &TextData,
    text_ui_size: vk::Extent2D,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<(Self, PendingDataInitialization), GPUDataAllocationError> {
    let ui = create_image(
      device,
      render_format,
      text_ui_size.width,
      text_ui_size.height,
      vk::ImageUsageFlags::COLOR_ATTACHMENT.bitor(vk::ImageUsageFlags::SAMPLED),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text UI",
    )?;

    let sprite_buffers = SpriteBuffers::new(
      device,
      sprite_texture_extent,
      render_format,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { destroy!(device => &ui) })?;

    let text_buffers = TextBuffers::new(
      device,
      text_data,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { destroy!(device => &sprite_buffers, &ui) })?;

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
        &ui,
        &text_buffers.curve_texture,
        &text_buffers.band_texture,
        &text_buffers.vertices,
        &text_buffers.indices,
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
        "Text vertices",
        "Text indices",
      ]),
      #[cfg(feature = "log_alloc")]
      "Constant allocation data",
    )
    .on_err(|_| unsafe { destroy!(device => &text_buffers, &sprite_buffers, &ui) })?;

    let pending_device_init = create_and_copy_from_staging_buffers(
      device,
      physical_device,
      queues,
      &sprite_buffers,
      sprite_texture_extent,
      sprite_texture_data,
      text_data,
      &text_buffers,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe {
      destroy!(device => &text_buffers, &sprite_buffers, &ui, &device_alloc)
    })?;

    let memories = device_alloc.get_memories().to_vec();
    log::info!("Allocated memory count: {}", memories.len());

    let views = ImageViews::new(device, render_format, &sprite_buffers, &text_buffers, ui)
      .on_err(|_| unsafe {destroy!(device => &pending_device_init, &text_buffers, &sprite_buffers, &ui, memories.as_slice()) })?;

    Ok((
      Self {
        sprite_buffers,
        sprite_view: views.sprite,
        memories,
        text: text_buffers,
        text_band_view: views.text_band,
        text_curve_view: views.text_curve,
        text_size: text_data.size,
        ui,
        ui_view: views.ui,
        ui_size: text_ui_size,
      },
      pending_device_init,
    ))
  }
}

impl DeviceManuallyDestroyed for SpriteBuffers {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.texture.destroy_self(device);
    self.quad_vertices.destroy_self(device);
    self.quad_indices.destroy_self(device);
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

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.sprite_view.destroy_self(device);
    self.text_curve_view.destroy_self(device);
    self.text_band_view.destroy_self(device);
    self.ui_view.destroy_self(device);

    self.ui.destroy_self(device);
    self.sprite_buffers.destroy_self(device);
    self.text.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
