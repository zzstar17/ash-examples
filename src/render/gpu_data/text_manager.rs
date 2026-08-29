use std::ptr;

use ash::vk;
use vkallocator::{HostMemorySyncError, MappedHostBuffer};
use vkobjects::DeviceManuallyDestroyed;

use crate::{
  font,
  last_frames_durations::FPSDurations,
  render::{
    command_pools::{self, graphics::GraphicsCommandBufferPool},
    gpu_data::{
      text_buffers::{TextBufferDimensions, TextBuffers},
      GPUDataAllocationError,
    },
  },
  slug::{MultilineRect, SlugRendering, SlugVertex, TextBuildBounds},
};

pub struct TextManager {
  pub slug: SlugRendering<'static>,
  pub buffers: TextBuffers,

  pub device_vertices: Vec<SlugVertex>,
  pub device_indices: Vec<u32>,
  pub host_vertices: Vec<SlugVertex>,
  pub host_indices: Vec<u32>,

  pub device_index_count: u32,
  pub host_index_count: u32,

  fps_offset: vk::Offset2D,
  gpu_idle_offset: vk::Offset2D,
}

impl TextManager {
  const FONT_SIZE: usize = 30;

  pub fn new(
    device: &ash::Device,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<(Self, MultilineRect, u64), GPUDataAllocationError> {
    let shaper = font::SHAPER_DATA.shaper(&font::FONT_REF).build();
    let mut slug = SlugRendering::new(&font::FONT_FACE, shaper);

    slug.add_glyphs_in_str(" 0123456789,.;yesno");

    let mut device_vertices = Vec::new();
    let mut device_indices = Vec::new();
    let line_dist = slug.get_line_dist(1.5);
    let TextBuildBounds {
      rect: rect_fps,
      offset: fps_offset,
    } = slug.build_text(
      "fps: ",
      Self::FONT_SIZE,
      vk::Offset2D { x: 0, y: 0 },
      &mut device_vertices,
      &mut device_indices,
    );
    let TextBuildBounds {
      rect: rect_gpu_idle,
      offset: gpu_idle_offset,
    } = slug.build_text(
      "GPU idle: ",
      Self::FONT_SIZE,
      vk::Offset2D {
        x: 0,
        y: -line_dist,
      },
      &mut device_vertices,
      &mut device_indices,
    );
    let mut full_size = MultilineRect::from_line_rects(&[rect_fps, rect_gpu_idle]);

    // todo: fix full slug size calculations
    full_size.total.max.x += 10.0;
    full_size.total.max.y += 20.0;

    let textures = slug.get_texture_data();

    let text_vertices_size = (device_vertices.len() * size_of::<SlugVertex>()) as u64;
    let text_indices_size = (device_indices.len() * size_of::<u32>()) as u64;

    let dimensions = TextBufferDimensions {
      device_vertices_size: text_vertices_size,
      device_indices_size: text_indices_size,
      cpu_vertices_size: (140 * 4 * size_of::<SlugVertex>()) as u64, // todo
      cpu_indices_size: (140 * 6 * size_of::<u32>()) as u64,         // todo
    };

    let buffers = TextBuffers::new(
      device,
      &textures,
      dimensions,
      #[cfg(feature = "vl")]
      marker,
    )?;

    let device_staging_required = textures.curve_tex_size()
      + textures.band_tex_size()
      + dimensions.device_vertices_size
      + dimensions.device_indices_size;

    Ok((
      Self {
        slug,
        buffers,
        device_vertices,
        device_indices,
        host_vertices: Vec::new(),
        host_indices: Vec::new(),
        device_index_count: 0,
        host_index_count: 0,

        fps_offset,
        gpu_idle_offset,
      },
      full_size,
      device_staging_required,
    ))
  }

  pub fn device_vertices_size(&self) -> u64 {
    (self.device_vertices.len() * size_of::<SlugVertex>()) as u64
  }

  pub fn device_indices_size(&self) -> u64 {
    (self.device_indices.len() * size_of::<u32>()) as u64
  }

  pub fn host_vertices_size(&self) -> u64 {
    (self.host_vertices.len() * size_of::<SlugVertex>()) as u64
  }

  pub fn host_indices_size(&self) -> u64 {
    (self.host_indices.len() * size_of::<u32>()) as u64
  }

  pub fn write_and_record_full_device_text_data(
    &mut self,
    device: &ash::Device,
    staging_buffer: MappedHostBuffer<u8>,
    pool: &GraphicsCommandBufferPool,
  ) -> Result<(), HostMemorySyncError> {
    let vertices_size = self.device_vertices_size();
    let indices_size = self.device_indices_size();

    let textures = self.slug.get_texture_data();
    let curve_texture_size = textures.curve_tex_size();
    let band_texture_size = textures.band_tex_size();

    let curve_tex_offset = 0;
    let band_tex_offset = curve_texture_size;
    let vertices_offset = band_texture_size + curve_texture_size;
    let indices_offset = vertices_size + vertices_offset;
    let total_size = indices_size + indices_offset;

    assert!(total_size <= staging_buffer.buffer_size);

    let staging_ptr = staging_buffer.data_ptr;
    let memory_range = vk::MappedMemoryRange {
      memory: staging_buffer.memory,
      offset: staging_buffer.buffer_offset,
      size: total_size,
      ..Default::default()
    };
    unsafe {
      ptr::copy_nonoverlapping(
        textures.curve_tex_data.as_ptr() as *const u8,
        staging_ptr.add(curve_tex_offset as usize).as_ptr(),
        curve_texture_size as usize,
      );
      ptr::copy_nonoverlapping(
        textures.band_tex_data.as_ptr() as *const u8,
        staging_ptr.add(band_tex_offset as usize).as_ptr(),
        band_texture_size as usize,
      );
      ptr::copy_nonoverlapping(
        self.device_vertices.as_ptr() as *const u8,
        staging_ptr.add(vertices_offset as usize).as_ptr(),
        vertices_size as usize,
      );
      ptr::copy_nonoverlapping(
        self.device_indices.as_ptr() as *const u8,
        staging_ptr.add(indices_offset as usize).as_ptr(),
        indices_size as usize,
      );
      if !staging_buffer.mem_host_coherent {
        let ranges = [memory_range];
        device.flush_mapped_memory_ranges(&ranges)?;
      }
    };

    let cb = pool.main;
    unsafe {
      pool.record_copy_staging_buffer_to_image(
        device,
        staging_buffer.buffer,
        curve_tex_offset,
        self.buffers.curve_texture,
        self.buffers.curve_texture_extent,
        vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        vk::PipelineStageFlags2::FRAGMENT_SHADER,
        vk::AccessFlags2::SHADER_SAMPLED_READ,
        vk::PipelineStageFlags2::FRAGMENT_SHADER,
        vk::AccessFlags2::SHADER_SAMPLED_READ,
      );
      pool.record_copy_staging_buffer_to_image(
        device,
        staging_buffer.buffer,
        band_tex_offset,
        self.buffers.band_texture,
        self.buffers.band_texture_extent,
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
          buffer: self.buffers.device.vertices,
          offset: 0,
          size: vertices_size,
          ..Default::default()
        };
        let wait_before_index_copy = vk::BufferMemoryBarrier2 {
          src_access_mask: vk::AccessFlags2::INDEX_READ,
          dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
          src_stage_mask: vk::PipelineStageFlags2::INDEX_INPUT,
          dst_stage_mask: vk::PipelineStageFlags2::COPY,
          buffer: self.buffers.device.indices,
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
          staging_buffer.buffer,
          self.buffers.device.vertices,
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
          staging_buffer.buffer,
          self.buffers.device.indices,
          &[region],
        );
      }
      let wait_before_vertex_stage = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::VERTEX_ATTRIBUTE_READ,
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::VERTEX_ATTRIBUTE_INPUT,
        buffer: self.buffers.device.vertices,
        offset: 0,
        size: vertices_size,
        ..Default::default()
      };
      let wait_before_index_stage = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::INDEX_READ,
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::INDEX_INPUT,
        buffer: self.buffers.device.indices,
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

    self.device_index_count = self.device_indices.len() as u32;

    Ok(())
  }

  // buffers need to be initialized
  // buffers need not to be in use
  pub fn write_host_device_text_data(
    &mut self,
    device: &ash::Device,
    frame_i: usize,
    fps: FPSDurations,
  ) -> Result<(), HostMemorySyncError> {
    self.host_vertices.clear();
    self.host_indices.clear();

    let fps_text = format!("{:.2}, {:.2}, {:.2}", fps.min, fps.max, fps.average);
    self.slug.build_text(
      &fps_text,
      Self::FONT_SIZE,
      self.fps_offset,
      &mut self.host_vertices,
      &mut self.host_indices,
    );
    self.slug.build_text(
      "no",
      Self::FONT_SIZE,
      self.gpu_idle_offset,
      &mut self.host_vertices,
      &mut self.host_indices,
    );

    let vertices_size = self.host_vertices_size();
    let indices_size = self.host_indices_size();

    assert!(vertices_size <= self.buffers.host[frame_i].vertices.buffer_size);
    assert!(indices_size <= self.buffers.host[frame_i].indices.buffer_size);

    unsafe {
      ptr::copy_nonoverlapping(
        self.host_vertices.as_ptr(),
        self.buffers.host[frame_i].vertices.data_ptr.as_ptr(),
        self.host_vertices.len(),
      );
      ptr::copy_nonoverlapping(
        self.host_indices.as_ptr(),
        self.buffers.host[frame_i].indices.data_ptr.as_ptr(),
        self.host_indices.len(),
      );
      if !self.buffers.host[frame_i].vertices.mem_host_coherent {
        let memory_range = vk::MappedMemoryRange {
          memory: self.buffers.host[frame_i].vertices.memory,
          offset: 0,
          size: vertices_size,
          ..Default::default()
        };
        let ranges = [memory_range];
        device.flush_mapped_memory_ranges(&ranges)?;
      }
      if !self.buffers.host[frame_i].indices.mem_host_coherent {
        let memory_range = vk::MappedMemoryRange {
          memory: self.buffers.host[frame_i].indices.memory,
          offset: 0,
          size: indices_size,
          ..Default::default()
        };
        let ranges = [memory_range];
        device.flush_mapped_memory_ranges(&ranges)?;
      }
    };

    self.host_index_count = self.host_indices.len() as u32;

    Ok(())
  }
}

impl DeviceManuallyDestroyed for TextManager {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.buffers.destroy_self(device);
  }
}
