use std::ops::BitOr;

use crate::{
  render::{
    create_objs::{create_buffer, create_image},
    gpu_data::{GPUDataAllocationError, TEXTURE_USAGES},
  },
  slug::{self, SlugTextureData, SlugVertex},
};
use ash::vk;
use vkinitialization::device::Device;
use vkobjects::{destroy, utility::OnErr, DeviceManuallyDestroyed};

#[derive(Debug, Clone, Copy)]
pub struct TextBuffers {
  pub curve_texture: vk::Image,
  pub curve_texture_extent: vk::Extent2D,
  pub band_texture: vk::Image,
  pub band_texture_extent: vk::Extent2D,

  pub device: DeviceTextBuffers,
  pub cpu: CPUTextBuffers,
}

#[derive(Debug, Clone, Copy)]
pub struct CPUTextBuffers {
  pub vertices: vk::Buffer,
  pub indices: vk::Buffer,
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceTextBuffers {
  pub vertices: vk::Buffer,
  pub indices: vk::Buffer,
  pub cur_indices_count: u32,
}

pub struct TextData<'a> {
  pub textures: SlugTextureData<'a>,
  pub vertices: &'a [SlugVertex],
  pub indices: &'a [u32],
}

#[derive(Debug, Clone, Copy)]
pub struct TextBufferDimensions {
  pub device_vertices_size: u64,
  pub device_indices_size: u64,
  pub cpu_vertices_size: u64,
  pub cpu_indices_size: u64,
}

impl TextBuffers {
  pub const CURVES_FORMAT: vk::Format = vk::Format::R32G32B32A32_SFLOAT;
  pub const BANDS_FORMAT: vk::Format = vk::Format::R32G32B32A32_UINT;

  pub fn new<'a>(
    device: &Device,
    textures: &SlugTextureData<'a>,
    buffer_dimensions: TextBufferDimensions,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let curve_texture_extent = vk::Extent2D {
      width: slug::TEX_WIDTH as u32,
      height: textures.curve_tex_height as u32,
    };
    let band_texture_extent = vk::Extent2D {
      width: slug::TEX_WIDTH as u32,
      height: textures.band_tex_height as u32,
    };

    // todo: add device checking for this
    let curve_texture = create_image(
      device,
      Self::CURVES_FORMAT,
      curve_texture_extent.width,
      curve_texture_extent.height,
      TEXTURE_USAGES,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text curve texture",
    )?;
    let band_texture = create_image(
      device,
      Self::BANDS_FORMAT,
      band_texture_extent.width,
      band_texture_extent.height,
      TEXTURE_USAGES,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text band texture",
    )
    .on_err(|_| unsafe { curve_texture.destroy_self(device) })?;

    let device_buffers = DeviceTextBuffers::new(
      device,
      buffer_dimensions.device_vertices_size,
      buffer_dimensions.device_indices_size,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { destroy!(device => &band_texture, &curve_texture) })?;
    let cpu_buffers = CPUTextBuffers::new(
      device,
      buffer_dimensions.cpu_vertices_size,
      buffer_dimensions.cpu_indices_size,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe { destroy!(device => &device_buffers, &band_texture, &curve_texture) })?;

    Ok(Self {
      curve_texture,
      curve_texture_extent,
      band_texture,
      band_texture_extent,
      device: device_buffers,
      cpu: cpu_buffers,
    })
  }
}

impl DeviceTextBuffers {
  pub fn new(
    device: &Device,
    vertices_size: u64,
    indices_size: u64,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let vertices: vk::Buffer = create_buffer(
      device,
      vertices_size,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Device text vertices",
    )?;
    let indices: vk::Buffer = create_buffer(
      device,
      indices_size,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Device text indices",
    )
    .on_err(|_| unsafe { destroy!(device => &vertices) })?;

    Ok(Self {
      vertices,
      indices,
      cur_indices_count: 0,
    })
  }
}

impl CPUTextBuffers {
  pub fn new(
    device: &Device,
    vertices_size: u64,
    indices_size: u64,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let vertices: vk::Buffer = create_buffer(
      device,
      vertices_size,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"CPU text vertices",
    )?;
    let indices: vk::Buffer = create_buffer(
      device,
      indices_size,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"CPU text indices",
    )
    .on_err(|_| unsafe { destroy!(device => &vertices) })?;

    Ok(Self { vertices, indices })
  }
}

impl DeviceManuallyDestroyed for TextBuffers {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.curve_texture.destroy_self(device);
    self.band_texture.destroy_self(device);
    self.cpu.destroy_self(device);
    self.device.destroy_self(device);
  }
}

impl DeviceManuallyDestroyed for DeviceTextBuffers {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.vertices.destroy_self(device);
    self.indices.destroy_self(device);
  }
}

impl DeviceManuallyDestroyed for CPUTextBuffers {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.vertices.destroy_self(device);
    self.indices.destroy_self(device);
  }
}
