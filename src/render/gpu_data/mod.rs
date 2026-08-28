mod allocations;
pub mod sprite_buffers;
pub mod text;

use std::{ops::BitOr, ptr};

use crate::{
  render::{
    command_pools::{self, graphics::GraphicsCommandBufferPool},
    create_objs::{create_image, create_image_view},
    gpu_data::{
      sprite_buffers::{SpriteBuffers, SpriteTextureData},
      text::{TextBufferDimensions, TextBuffers, TextData},
    },
    render_object::{QUAD_INDICES, QUAD_INDICES_SIZE, VERTICES, VERTICES_SIZE},
  },
  slug::{self, MultilineRect, SlugTextureData, SlugVertex},
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

#[derive(Debug)]
pub struct GPUData {
  pub staging: MappedHostBuffer<u8>,

  pub sprite_buffers: SpriteBuffers,
  pub sprite_view: vk::ImageView,

  pub text: TextBuffers,
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
    text_textures: &SlugTextureData,
    text_buffer_dimensions: TextBufferDimensions,
    text_ui_size: vk::Extent2D,
    text_ui_rect: MultilineRect,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let text_ui = create_image(
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
      vk::Extent2D {
        width: sprite_texture_data.width,
        height: sprite_texture_data.height,
      },
      render_format,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { destroy!(device => &text_ui) })?;

    let text_buffers = TextBuffers::new(
      device,
      text_textures,
      text_buffer_dimensions,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { destroy!(device => &sprite_buffers, &text_ui) })?;

    let text_curve_len = slug::TEX_WIDTH * text_textures.curve_tex_height * 4;
    let text_band_len = slug::TEX_WIDTH * text_textures.band_tex_height * 4;

    let text_curve_texture_size = (text_curve_len * size_of::<f32>()) as u64;
    let text_band_texture_size = (text_band_len * size_of::<u32>()) as u64;
    assert_eq!(text_textures.curve_tex_data.len() * 4, text_curve_len);
    assert_eq!(text_textures.band_tex_data.len(), text_band_len);

    let staging_size = (sprite_texture_data.bytes.len() as u64 + VERTICES_SIZE + QUAD_INDICES_SIZE)
      .max(
        text_curve_texture_size
          + text_band_texture_size
          + text_buffer_dimensions.device_vertices_size
          + text_buffer_dimensions.device_indices_size,
      );

    let staging_alloc =
      allocations::allocate_staging_memory(device, physical_device, staging_size, marker)?;
    let device_alloc = allocations::allocate_device(
      device,
      physical_device,
      &sprite_buffers,
      &text_buffers,
      text_ui,
    )?;
    let host_device_alloc =
      allocations::allocate_host_device(device, physical_device, &text_buffers)?;

    let views = ImageViews::new(
      device,
      render_format,
      &sprite_buffers,
      &text_buffers,
      text_ui,
    )?;

    Ok(Self {
      staging: staging_alloc,

      sprite_buffers,
      sprite_view: views.sprite,

      text: text_buffers,
      text_band_view: views.text_band,
      text_curve_view: views.text_curve,

      text_ui,
      text_ui_rect,
      text_ui_view: views.ui,
      text_ui_size: text_ui_size,

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
    device: &Device,
    text_data: &TextData,
    pool: &GraphicsCommandBufferPool,
  ) -> Result<(), HostMemorySyncError> {
    let vertices_size = (text_data.vertices.len() * size_of::<SlugVertex>()) as u64;
    let indices_size = (text_data.indices.len() * size_of::<u32>()) as u64;

    let curve_len = slug::TEX_WIDTH * text_data.textures.curve_tex_height * 4;
    let band_len = slug::TEX_WIDTH * text_data.textures.band_tex_height * 4;

    let curve_texture_size = (curve_len * size_of::<f32>()) as u64;
    let band_texture_size = (band_len * size_of::<u32>()) as u64;

    let curve_tex_offset = 0;
    let band_tex_offset = curve_texture_size;
    let vertices_offset = band_texture_size + curve_texture_size;
    let indices_offset = vertices_size + vertices_offset;
    let initial_copy_size = indices_size + indices_offset;
    println!(
      "OFFSETS: {} {} {} {}, {}",
      curve_tex_offset, band_tex_offset, vertices_offset, indices_offset, initial_copy_size
    );

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
        text_data.textures.curve_tex_data.as_ptr() as *const u8,
        staging_ptr.add(curve_tex_offset as usize).as_ptr(),
        curve_texture_size as usize,
      );
      ptr::copy_nonoverlapping(
        text_data.textures.band_tex_data.as_ptr() as *const u8,
        staging_ptr.add(band_tex_offset as usize).as_ptr(),
        band_texture_size as usize,
      );
      ptr::copy_nonoverlapping(
        text_data.vertices.as_ptr() as *const u8,
        staging_ptr.add(vertices_offset as usize).as_ptr(),
        vertices_size as usize,
      );
      ptr::copy_nonoverlapping(
        text_data.indices.as_ptr() as *const u8,
        staging_ptr.add(indices_offset as usize).as_ptr(),
        indices_size as usize,
      );
      if !self.staging.mem_host_coherent {
        let ranges = [memory_range];
        device.flush_mapped_memory_ranges(&ranges)?;
      }
    };

    let cb = pool.main;
    unsafe {
      pool.record_copy_staging_buffer_to_image(
        device,
        self.staging.buffer,
        curve_tex_offset,
        self.text.curve_texture,
        self.text.curve_texture_extent,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        vk::PipelineStageFlags2::FRAGMENT_SHADER,
        vk::AccessFlags2::SHADER_SAMPLED_READ,
        vk::PipelineStageFlags2::FRAGMENT_SHADER,
        vk::AccessFlags2::SHADER_SAMPLED_READ,
      );
      pool.record_copy_staging_buffer_to_image(
        device,
        self.staging.buffer,
        band_tex_offset,
        self.text.band_texture,
        self.text.band_texture_extent,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        vk::PipelineStageFlags2::FRAGMENT_SHADER,
        vk::AccessFlags2::SHADER_SAMPLED_READ,
        vk::PipelineStageFlags2::FRAGMENT_SHADER,
        vk::AccessFlags2::SHADER_SAMPLED_READ,
      );
      {
        let wait_before_vertex_copy = vk::BufferMemoryBarrier2 {
          src_access_mask: vk::AccessFlags2::VERTEX_ATTRIBUTE_READ,
          dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
          src_stage_mask: vk::PipelineStageFlags2::VERTEX_ATTRIBUTE_INPUT,
          dst_stage_mask: vk::PipelineStageFlags2::COPY,
          buffer: self.text.device.vertices,
          offset: 0,
          size: vertices_size,
          ..Default::default()
        };
        let wait_before_index_copy = vk::BufferMemoryBarrier2 {
          src_access_mask: vk::AccessFlags2::INDEX_READ,
          dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
          src_stage_mask: vk::PipelineStageFlags2::INDEX_INPUT,
          dst_stage_mask: vk::PipelineStageFlags2::COPY,
          buffer: self.text.device.indices,
          offset: 0,
          size: indices_size,
          ..Default::default()
        };
        device.cmd_pipeline_barrier2(
          cb,
          &command_pools::dependency_info(
            &[],
            &[wait_before_vertex_copy, wait_before_index_copy],
            &[],
          ),
        );
      }
      {
        let region = vk::BufferCopy {
          src_offset: vertices_offset,
          dst_offset: 0,
          size: vertices_size,
        };
        device.cmd_copy_buffer(
          cb,
          self.staging.buffer,
          self.text.device.vertices,
          &[region],
        );
      }
      {
        let region = vk::BufferCopy {
          src_offset: indices_offset,
          dst_offset: 0,
          size: indices_size,
        };
        device.cmd_copy_buffer(cb, self.staging.buffer, self.text.device.indices, &[region]);
      }
      let wait_before_vertex_stage = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::VERTEX_ATTRIBUTE_READ,
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::VERTEX_ATTRIBUTE_INPUT,
        buffer: self.text.device.vertices,
        offset: 0,
        size: vertices_size,
        ..Default::default()
      };
      let wait_before_index_stage = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::INDEX_READ,
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::INDEX_INPUT,
        buffer: self.text.device.indices,
        offset: 0,
        size: indices_size,
        ..Default::default()
      };
      device.cmd_pipeline_barrier2(
        cb,
        &command_pools::dependency_info(
          &[],
          &[wait_before_vertex_stage, wait_before_index_stage],
          &[],
        ),
      );
    }

    self.text.device.cur_indices_count = text_data.indices.len() as u32;

    Ok(())
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
