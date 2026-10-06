mod allocations;
pub mod sprite_buffers;
pub mod text_buffers;
pub mod text_manager;

use std::{ops::BitOr, ptr};

use crate::{
  asset_loader::{texture_loader::TextureData, LoadedModels},
  render::{
    command_pools::graphics::GraphicsCommandBufferPool,
    create_objs::{create_color_image_view, create_image, create_image_sampled},
    gpu_data::{
      sprite_buffers::SpriteBuffers, text_buffers::TextBuffers, text_manager::TextManager,
    },
  },
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

  pub text_ui_multisampled: vk::ImageView,
  pub text_ui: vk::ImageView,
}

pub struct GPUData {
  pub staging: MappedHostBuffer<u8>,

  pub sprite_buffers: SpriteBuffers,
  pub sprite_view: vk::ImageView,

  pub text: TextManager,
  pub text_curve_view: vk::ImageView,
  pub text_band_view: vk::ImageView,

  pub text_ui_multisampled: vk::Image,
  pub text_ui: vk::Image,
  pub text_ui_line_size: f32,
  pub text_ui_multisampled_view: vk::ImageView,
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
    text_ui_multisampled: vk::Image,
    text_ui: vk::Image,
  ) -> Result<Self, OutOfMemoryError> {
    let sprite = create_color_image_view(
      device,
      sprite_buffers.texture,
      render_format,
      sprite_buffers.texture_mip_levels,
    )?;

    let text_curve = create_color_image_view(
      device,
      text_buffers.curve_texture,
      TextBuffers::CURVES_FORMAT,
      1,
    )
    .on_err(|_| unsafe { destroy!(device => &sprite) })?;
    let text_band = create_color_image_view(
      device,
      text_buffers.band_texture,
      TextBuffers::BANDS_FORMAT,
      1,
    )
    .on_err(|_| unsafe { destroy!(device => &text_curve, &sprite) })?;

    let text_ui_multisampled =
      create_color_image_view(device, text_ui_multisampled, render_format, 1)
        .on_err(|_| unsafe { destroy!(device => &text_band, &text_curve, &sprite) })?;

    let text_ui = create_color_image_view(device, text_ui, render_format, 1)
      .on_err(|_| unsafe { destroy!(device => &text_band, &text_curve, &sprite) })?;

    Ok(Self {
      sprite,
      text_curve,
      text_band,
      text_ui_multisampled,
      text_ui,
    })
  }
}

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    render_format: vk::Format,
    loaded_models: &LoadedModels,
    texture_data: &TextureData,
    sample_count: usize,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let sprite_buffers = SpriteBuffers::new(
      device,
      loaded_models,
      vk::Extent2D {
        width: texture_data.width,
        height: texture_data.height,
      },
      render_format,
      texture_data.reader.header().level_count.max(1),
      #[cfg(feature = "vl")]
      marker,
    )?;

    let (mut text_manager, (line_size, text_window_rect), staging_size_required) =
      TextManager::new(
        device,
        #[cfg(feature = "vl")]
        marker,
      )
      .on_err(|_| unsafe { destroy!(device => &sprite_buffers) })?;
    let text_extent = text_window_rect.into_vk_extent();

    let text_ui_multisampled = create_image_sampled(
      device,
      render_format,
      text_extent.width,
      text_extent.height,
      sample_count,
      vk::ImageUsageFlags::COLOR_ATTACHMENT,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text UI",
    )
    .on_err(|_| unsafe { destroy!(device => &text_manager, &sprite_buffers) })?;

    let text_ui = create_image(
      device,
      render_format,
      text_extent.width,
      text_extent.height,
      1,
      vk::ImageUsageFlags::COLOR_ATTACHMENT.bitor(vk::ImageUsageFlags::SAMPLED),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text UI",
    )
    .on_err(|_| unsafe {
      destroy!(device => &text_ui_multisampled, &text_manager, &sprite_buffers)
    })?;

    let staging_size = (texture_data.total_mip_levels_size
      + loaded_models.vertices_size()
      + loaded_models.indices_size())
    .max(staging_size_required);

    let staging_alloc = allocations::allocate_staging_memory(
      device,
      physical_device,
      staging_size,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe {
      destroy!(device => &text_ui, &text_ui_multisampled, &text_manager, &sprite_buffers)
    })?;
    let device_alloc = allocations::allocate_device(
      device,
      physical_device,
      &sprite_buffers,
      &text_manager.buffers,
      text_ui_multisampled,
      text_ui,
    ).on_err(|_| unsafe {
      destroy!(device => &text_ui, &text_ui_multisampled, &text_manager, &sprite_buffers, &staging_alloc)
    })?;
    let device_alloc_ref: &[DetailedMemory] = &device_alloc;
    let host_device_alloc =
      allocations::allocate_host_device(device, physical_device, &mut text_manager.buffers).on_err(|_| unsafe {
      destroy!(device => &text_ui, &text_ui_multisampled, &text_manager, &sprite_buffers, &staging_alloc, device_alloc_ref)
    })?;
    let host_device_alloc_ref: &[DetailedMemory] = &host_device_alloc;

    let views: ImageViews = ImageViews::new(
      device,
      render_format,
      &sprite_buffers,
      &text_manager.buffers,
      text_ui_multisampled,
      text_ui,
    ).on_err(|_| unsafe {
      destroy!(device => &text_ui, &text_ui_multisampled, &text_manager, &sprite_buffers, &staging_alloc, device_alloc_ref, host_device_alloc_ref)
    })?;

    Ok(Self {
      staging: staging_alloc,

      sprite_buffers,
      sprite_view: views.sprite,

      text: text_manager,
      text_band_view: views.text_band,
      text_curve_view: views.text_curve,

      text_ui_multisampled,
      text_ui,
      text_ui_line_size: line_size,
      text_ui_multisampled_view: views.text_ui_multisampled,
      text_ui_view: views.text_ui,
      text_ui_size: text_extent,

      device_memories: device_alloc,
      host_device_memories: host_device_alloc,
    })
  }

  pub fn write_and_record_initial_staging_data(
    &self,
    device: &Device,
    loaded_models: &LoadedModels,
    texture_data: &TextureData,
    pool: &GraphicsCommandBufferPool,
  ) -> Result<(), HostMemorySyncError> {
    let vertices_size = loaded_models.vertices_size();
    let indices_size = loaded_models.indices_size();

    let vertices_offset = 0;
    let indices_offset = vertices_size;
    let texture_offset = indices_size + indices_offset;
    let initial_copy_size = texture_offset + texture_data.total_mip_levels_size;

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
        loaded_models.vertices.as_ptr() as *const u8,
        staging_ptr.add(vertices_offset as usize).as_ptr(),
        vertices_size as usize,
      );
      ptr::copy_nonoverlapping(
        loaded_models.indices.as_ptr() as *const u8,
        staging_ptr.add(indices_offset as usize).as_ptr(),
        indices_size as usize,
      );

      let mut local_level_offset = 0;
      let staging_texture_offset = staging_ptr.add(texture_offset as usize);
      for level in texture_data.reader.levels() {
        assert_eq!(level.data.len(), level.uncompressed_byte_length as usize);
        ptr::copy_nonoverlapping(
          level.data.as_ptr(),
          staging_texture_offset.add(local_level_offset).as_ptr(),
          level.uncompressed_byte_length as usize,
        );
        local_level_offset += level.uncompressed_byte_length as usize;
      }

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
          size: vertices_size,
        };
        device.cmd_copy_buffer(
          cb,
          self.staging.buffer,
          self.sprite_buffers.vertices,
          &[region],
        );
      }
      {
        let region = vk::BufferCopy {
          src_offset: indices_offset,
          dst_offset: 0,
          size: indices_size,
        };
        device.cmd_copy_buffer(
          cb,
          self.staging.buffer,
          self.sprite_buffers.indices,
          &[region],
        );
      }
      pool.record_copy_staging_buffer_to_image_multisample(
        device,
        self.staging.buffer,
        texture_offset,
        self.sprite_buffers.texture,
        self.sprite_buffers.texture_extent,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        // fence flushes all memory
        vk::PipelineStageFlags2::NONE,
        vk::AccessFlags2::NONE,
        vk::PipelineStageFlags2::NONE,
        vk::AccessFlags2::NONE,
        &mut texture_data.reader.levels(),
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
    self.text_ui_multisampled_view.destroy_self(device);
    self.text_ui_view.destroy_self(device);

    self.text_ui_multisampled.destroy_self(device);
    self.text_ui.destroy_self(device);
    self.sprite_buffers.destroy_self(device);
    self.text.destroy_self(device);

    self.staging.buffer.destroy_self(device);
    self.staging.memory.destroy_self(device);

    self.device_memories.destroy_self(device);
    self.host_device_memories.destroy_self(device);
  }
}
