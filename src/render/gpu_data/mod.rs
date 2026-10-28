mod allocations;
pub mod sprite_buffers;
pub mod text_buffers;
pub mod text_manager;

use std::{ops::BitOr, ptr};

use crate::{
  asset_loader::{model_loader::ModelGpuData, texture_loader::LoadedImageData},
  render::{
    command_pools::graphics::GraphicsCommandBufferPool,
    create_objs::{create_color_image_view, create_image, create_image_sampled},
    format_conversions,
    gpu_data::{
      sprite_buffers::SpriteBuffers, text_buffers::TextBuffers, text_manager::TextManager,
    },
  },
};
use ash::vk;
use itertools::Itertools;
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

struct ImageViews {
  pub text_curve: vk::ImageView,
  pub text_band: vk::ImageView,

  pub text_ui_render: Option<vk::ImageView>,
  pub text_ui_resolved: vk::ImageView,
}

#[derive(Debug, Clone, Copy)]
struct TextUIImages {
  pub render: Option<vk::Image>,
  pub resolved: vk::Image,
}

pub struct TextUI {
  // none if multisampling is 1 (disabled)
  // multisampled color image
  pub render: Option<vk::Image>,
  pub render_view: Option<vk::ImageView>,

  // resolve color images with 1 sample
  // directly rendered to if multisampling is disabled
  // used after rendering
  pub resolved: vk::Image,
  pub resolved_view: vk::ImageView,

  pub line_size: f32,
  pub size: vk::Extent2D,
}

pub struct GPUData {
  pub staging: MappedHostBuffer<u8>,

  pub sprite_buffers: SpriteBuffers,

  pub text: TextManager,
  pub text_curve_view: vk::ImageView,
  pub text_band_view: vk::ImageView,
  pub text_ui: TextUI,

  device_memories: Vec<DetailedMemory>,
  host_device_memories: Vec<DetailedMemory>,
}

impl ImageViews {
  pub fn new(
    device: &Device,
    render_format: vk::Format,
    text_buffers: &TextBuffers,
    text_ui: TextUIImages,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, OutOfMemoryError> {
    let text_curve = create_color_image_view(
      device,
      text_buffers.curve_texture,
      TextBuffers::CURVES_FORMAT,
      1,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text Curve view",
    )?;
    let text_band = create_color_image_view(
      device,
      text_buffers.band_texture,
      TextBuffers::BANDS_FORMAT,
      1,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text Band view",
    )
    .on_err(|_| unsafe { destroy!(device => &text_curve) })?;

    let text_ui_render = if let Some(image) = text_ui.render {
      let view = create_color_image_view(
        device,
        image,
        render_format,
        1,
        #[cfg(feature = "vl")]
        marker,
        #[cfg(feature = "vl")]
        c"Text UI view (multisampled)",
      )
      .on_err(|_| unsafe { destroy!(device => &text_band, &text_curve) })?;
      Some(view)
    } else {
      None
    };

    let text_ui_resolved = create_color_image_view(
      device,
      text_ui.resolved,
      render_format,
      1,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text UI view (resolved)",
    )
    .on_err(|_| unsafe { destroy!(device => &text_ui_render, &text_band, &text_curve) })?;

    Ok(Self {
      text_curve,
      text_band,
      text_ui_render,
      text_ui_resolved,
    })
  }
}

impl TextUIImages {
  pub fn new(
    device: &Device,
    render_format: vk::Format,
    text_extent: vk::Extent2D,
    sample_count: usize,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, OutOfMemoryError> {
    let render = if sample_count > 1 {
      let image = create_image_sampled(
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
      )?;
      Some(image)
    } else {
      None
    };

    let resolved = create_image(
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
    .on_err(|_| unsafe { destroy!(device => &render) })?;

    Ok(Self { render, resolved })
  }
}

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    texture_format: vk::Format,
    render_format: vk::Format,
    models: &mut ModelGpuData,
    sample_count: usize,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let mut sprite_buffers = SpriteBuffers::new(
      device,
      models,
      texture_format,
      // texture_data.reader.header().level_count.max(1),
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

    let text_ui_images = TextUIImages::new(
      device,
      render_format,
      text_extent,
      sample_count,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { destroy!(device => &text_manager, &sprite_buffers) })?;

    let textures_size: u64 = sprite_buffers.textures.iter().map(|t| t.total_size).sum();
    let staging_size =
      (textures_size + models.vertices_size() + models.indices_size()).max(staging_size_required);

    let staging_alloc = allocations::allocate_staging_memory(
      device,
      physical_device,
      staging_size,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { destroy!(device => &text_ui_images, &text_manager, &sprite_buffers) })?;
    let device_alloc = allocations::allocate_device(
      device,
      physical_device,
      &sprite_buffers,
      &text_manager.buffers,
      text_ui_images,
    )
    .on_err(|_| unsafe {
      destroy!(device => &text_ui_images, &text_manager, &sprite_buffers, &staging_alloc)
    })?;
    let device_alloc_ref: &[DetailedMemory] = &device_alloc;
    let host_device_alloc =
      allocations::allocate_host_device(device, physical_device, &mut text_manager.buffers).on_err(|_| unsafe {
      destroy!(device => &text_ui_images, &text_manager, &sprite_buffers, &staging_alloc, device_alloc_ref)
    })?;
    let host_device_alloc_ref: &[DetailedMemory] = &host_device_alloc;

    let mut texture_err = None;
    for texture in sprite_buffers.textures.iter_mut() {
      let view_res = create_color_image_view(
        device,
        texture.image,
        texture_format,
        texture.mip_levels,
        #[cfg(feature = "vl")]
        marker,
        #[cfg(feature = "vl")]
        c"Texture view",
      );
      match view_res {
        Err(err) => {
          texture_err = Some(err);
        }
        Ok(view) => {
          texture.view = view;
        }
      }
    }
    if let Some(err) = texture_err {
      unsafe {
        destroy!(device => &text_ui_images, &text_manager, &sprite_buffers, &staging_alloc, device_alloc_ref, host_device_alloc_ref);
      }
      return Err(err.into());
    }

    let views: ImageViews = ImageViews::new(
      device,
      render_format,
      &text_manager.buffers,
      text_ui_images,
      #[cfg(feature = "vl")] marker,
    ).on_err(|_| unsafe {
      destroy!(device => &text_ui_images, &text_manager, &sprite_buffers, &staging_alloc, device_alloc_ref, host_device_alloc_ref)
    })?;

    let text_ui = TextUI {
      render: text_ui_images.render,
      render_view: views.text_ui_render,
      resolved: text_ui_images.resolved,
      resolved_view: views.text_ui_resolved,

      line_size,
      size: text_extent,
    };

    Ok(Self {
      staging: staging_alloc,

      sprite_buffers,

      text: text_manager,
      text_band_view: views.text_band,
      text_curve_view: views.text_curve,
      text_ui,

      device_memories: device_alloc,
      host_device_memories: host_device_alloc,
    })
  }

  pub fn write_and_record_initial_staging_data(
    &self,
    device: &Device,
    models: &mut ModelGpuData,
    pool: &GraphicsCommandBufferPool,
  ) -> Result<(), HostMemorySyncError> {
    let vertices_size = models.vertices_size();
    let indices_size = models.indices_size();
    let textures_size: u64 = self
      .sprite_buffers
      .textures
      .iter()
      .map(|t| t.total_size)
      .sum();

    let vertices_offset = 0;
    let indices_offset = vertices_size;
    let textures_offset = indices_size + indices_offset;
    let initial_copy_size = textures_offset + textures_size;

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
        models.vertices.as_ptr() as *const u8,
        staging_ptr.add(vertices_offset as usize).as_ptr(),
        vertices_size as usize,
      );
      ptr::copy_nonoverlapping(
        models.indices.as_ptr() as *const u8,
        staging_ptr.add(indices_offset as usize).as_ptr(),
        indices_size as usize,
      );

      let mut level_copy_buffer = Vec::new();
      let mut cur_texture_offset = textures_offset;
      for (image, texture) in models
        .images
        .iter()
        .zip_eq(self.sprite_buffers.textures.iter())
      {
        let staging_texture_offset = staging_ptr.add(cur_texture_offset as usize);
        let mut local_level_offset = 0;
        for level in image.reader.levels() {
          assert_eq!(level.data.len(), level.uncompressed_byte_length as usize);

          // convert data if format is not RGBA
          let data = level.data;
          let converted_data = if LoadedImageData::TEXTURE_FORMAT != texture.format {
            level_copy_buffer.clear();
            level_copy_buffer.extend_from_slice(data);
            format_conversions::convert_rgba_data_to_format(&mut level_copy_buffer, texture.format);
            &level_copy_buffer
          } else {
            data
          };
          ptr::copy_nonoverlapping(
            converted_data.as_ptr(),
            staging_texture_offset.add(local_level_offset).as_ptr(),
            level.uncompressed_byte_length as usize,
          );
          local_level_offset += level.uncompressed_byte_length as usize;
        }
        cur_texture_offset += texture.total_size;
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

      let mut cur_texture_offset = textures_offset;
      for (image, texture) in models
        .images
        .iter()
        .zip_eq(self.sprite_buffers.textures.iter())
      {
        pool.record_copy_staging_buffer_to_image_multisample(
          device,
          self.staging.buffer,
          cur_texture_offset,
          texture.image,
          texture.extent,
          vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
          // fence flushes all memory
          vk::PipelineStageFlags2::NONE,
          vk::AccessFlags2::NONE,
          vk::PipelineStageFlags2::NONE,
          vk::AccessFlags2::NONE,
          &mut image.reader.levels(),
        );

        cur_texture_offset += texture.total_size;
      }
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

impl DeviceManuallyDestroyed for TextUI {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.render_view.destroy_self(device);
    self.resolved_view.destroy_self(device);

    self.render.destroy_self(device);
    self.resolved.destroy_self(device);
  }
}

impl DeviceManuallyDestroyed for TextUIImages {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.render.destroy_self(device);
    self.resolved.destroy_self(device);
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.text_curve_view.destroy_self(device);
    self.text_band_view.destroy_self(device);

    self.text_ui.destroy_self(device);
    self.sprite_buffers.destroy_self(device);
    self.text.destroy_self(device);

    self.staging.buffer.destroy_self(device);
    self.staging.memory.destroy_self(device);

    self.device_memories.destroy_self(device);
    self.host_device_memories.destroy_self(device);
  }
}
