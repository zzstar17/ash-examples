mod allocations;
pub mod sprite_buffers;
pub mod text_buffers;
pub mod text_manager;

use std::{ops::BitOr, ptr};

use crate::{
  render::{
    command_pools::GraphicsCommandBufferPool,
    create_objs::{create_image, create_image_view},
    gpu_data::{
      sprite_buffers::{SpriteBuffers, SpriteTextureData},
      text_buffers::TextBuffers,
      text_manager::TextManager,
    },
    vertices::{QUAD_INDICES, QUAD_INDICES_SIZE, VERTICES, VERTICES_SIZE},
  },
  slug::MultilineRect,
};
use ash::vk;
use vkinitialization::device::{Device, PhysicalDevice};
use vkobjects::{
  const_flag_bitor, destroy,
  errors::{OutOfMemoryError, QueueSubmitError},
  utility::OnErr,
  DeviceManuallyDestroyed,
};

use vkallocator::{
  AllocationError, DetailedMemory, DeviceMemoryInitializationError, HostAllocationError,
  HostMemorySyncError, MappedHostBuffer,
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

pub struct ImageViews {
  pub sprite: vk::ImageView,
  pub text_curve: vk::ImageView,
  pub text_band: vk::ImageView,
  pub ui: vk::ImageView,
}

pub struct GPUData {
  pub staging: MappedHostBuffer<u8>,

  pub sprite_buffers: SpriteBuffers,
  pub sprite_view: vk::ImageView,

  pub text: TextManager,
  pub text_curve_view: vk::ImageView,
  pub text_band_view: vk::ImageView,

  pub text_ui: vk::Image,
  pub text_ui_rect: MultilineRect,
  pub text_ui_view: vk::ImageView,
  pub text_ui_size: vk::Extent2D,

  device_memories: Vec<DetailedMemory>,
  host_device_memories: Vec<DetailedMemory>,
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

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    render_format: vk::Format,
    sprite_texture_data: &SpriteTextureData,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let sprite_buffers = SpriteBuffers::new(
      device,
      vk::Extent2D {
        width: sprite_texture_data.width,
        height: sprite_texture_data.height,
      },
      render_format,
      #[cfg(feature = "vl")]
      marker,
    )?;

    let (mut text_manager, text_window_rect, staging_size_required) = TextManager::new(
      device,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { destroy!(device => &sprite_buffers) })?;
    let text_extent = text_window_rect.total.to_vk_extent();

    let text_ui = create_image(
      device,
      render_format,
      text_extent.width,
      text_extent.height,
      vk::ImageUsageFlags::COLOR_ATTACHMENT.bitor(vk::ImageUsageFlags::SAMPLED),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text UI",
    )?;

    let staging_size = (sprite_texture_data.bytes.len() as u64 + VERTICES_SIZE + QUAD_INDICES_SIZE)
      .max(staging_size_required);

    let staging_alloc = allocations::allocate_staging_memory(
      device,
      physical_device,
      staging_size,
      #[cfg(feature = "vl")]
      marker,
    )?;
    let device_alloc = allocations::allocate_device(
      device,
      physical_device,
      &sprite_buffers,
      &text_manager.buffers,
      text_ui,
    )?;
    let host_device_alloc =
      allocations::allocate_host_device(device, physical_device, &mut text_manager.buffers)?;

    let views = ImageViews::new(
      device,
      render_format,
      &sprite_buffers,
      &text_manager.buffers,
      text_ui,
    )?;

    Ok(Self {
      staging: staging_alloc,

      sprite_buffers,
      sprite_view: views.sprite,

      text: text_manager,
      text_band_view: views.text_band,
      text_curve_view: views.text_curve,

      text_ui,
      text_ui_rect: text_window_rect,
      text_ui_view: views.ui,
      text_ui_size: text_extent,

      device_memories: device_alloc,
      host_device_memories: host_device_alloc,
    })
  }

  pub fn write_and_record_initial_staging_data(
    &self,
    device: &Device,
    sprite_texture_bytes: &[u8],
    pool: &GraphicsCommandBufferPool,
  ) -> Result<(), HostMemorySyncError> {
    let sprite_texture_data_size = sprite_texture_bytes.len() as u64;

    let vertices_offset = 0;
    let indices_offset = VERTICES_SIZE;
    let sprite_texture_offset = QUAD_INDICES_SIZE + indices_offset;
    let initial_copy_size = sprite_texture_offset + sprite_texture_data_size;

    assert!(initial_copy_size <= self.staging.buffer_size);

    let staging_ptr = self.staging.data_ptr;
    let memory_range = vk::MappedMemoryRange {
      memory: self.staging.memory,
      offset: self.staging.buffer_offset,
      size: initial_copy_size,
      ..Default::default()
    };
    unsafe {
      ptr::copy_nonoverlapping(
        VERTICES.as_ptr() as *const u8,
        staging_ptr.add(vertices_offset as usize).as_ptr(),
        VERTICES_SIZE as usize,
      );
      ptr::copy_nonoverlapping(
        QUAD_INDICES.as_ptr() as *const u8,
        staging_ptr.add(indices_offset as usize).as_ptr(),
        QUAD_INDICES_SIZE as usize,
      );
      ptr::copy_nonoverlapping(
        sprite_texture_bytes.as_ptr(),
        staging_ptr.add(sprite_texture_offset as usize).as_ptr(),
        sprite_texture_data_size as usize,
      );
      if !self.staging.mem_host_coherent {
        let ranges = [memory_range];
        device.flush_mapped_memory_ranges(&ranges)?;
      }
    };

    let cb = pool.main;
    unsafe {
      {
        let region = vk::BufferCopy {
          src_offset: vertices_offset,
          dst_offset: 0,
          size: VERTICES_SIZE,
        };
        device.cmd_copy_buffer(
          cb,
          self.staging.buffer,
          self.sprite_buffers.quad_vertices,
          &[region],
        );
      }
      {
        let region = vk::BufferCopy {
          src_offset: indices_offset,
          dst_offset: 0,
          size: QUAD_INDICES_SIZE,
        };
        device.cmd_copy_buffer(
          cb,
          self.staging.buffer,
          self.sprite_buffers.quad_indices,
          &[region],
        );
      }
      pool.record_copy_staging_buffer_to_image(
        device,
        self.staging.buffer,
        sprite_texture_offset,
        self.sprite_buffers.texture,
        self.sprite_buffers.texture_extent,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        // fence flushes all memory
        vk::PipelineStageFlags2::NONE,
        vk::AccessFlags2::NONE,
        vk::PipelineStageFlags2::NONE,
        vk::AccessFlags2::NONE,
      );
    }

    Ok(())
  }

  pub fn write_and_record_full_device_text_data(
    &mut self,
    device: &ash::Device,
    pool: &GraphicsCommandBufferPool,
  ) -> Result<(), HostMemorySyncError> {
    self
      .text
      .write_and_record_full_device_text_data(device, self.staging, pool)
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.sprite_view.destroy_self(device);
    self.text_curve_view.destroy_self(device);
    self.text_band_view.destroy_self(device);
    self.text_ui_view.destroy_self(device);

    self.text_ui.destroy_self(device);
    self.sprite_buffers.destroy_self(device);
    self.text.destroy_self(device);

    self.staging.buffer.destroy_self(device);
    self.staging.memory.destroy_self(device);

    self.device_memories.destroy_self(device);
    self.host_device_memories.destroy_self(device);
  }
}
