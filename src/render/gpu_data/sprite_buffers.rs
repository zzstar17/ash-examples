use std::ops::BitOr;

use ash::vk;
use vkinitialization::device::Device;
use vkobjects::{destroy, utility::OnErr, DeviceManuallyDestroyed};

use crate::{
  render::{
    create_objs::{create_buffer, create_image},
    gpu_data::{GPUDataAllocationError, TEXTURE_USAGES},
    render_object::{QUAD_INDICES_SIZE, VERTICES_SIZE},
  },
  TEXTURE_PATH,
};

pub struct SpriteTextureData {
  pub bytes: Vec<u8>,
  pub width: u32,
  pub height: u32,
}

impl SpriteTextureData {
  pub fn read_texture_bytes_as_rgba8() -> Result<Self, image::ImageError> {
    let img = image::ImageReader::open(TEXTURE_PATH)?
      .decode()?
      .into_rgba8();
    let width = img.width();
    let height = img.height();

    let bytes = img.into_raw();
    assert!(bytes.len() == width as usize * height as usize * 4);
    Ok(Self {
      bytes,
      width,
      height,
    })
  }
}

#[derive(Debug, Clone, Copy)]
pub struct SpriteBuffers {
  pub texture: vk::Image,
  pub texture_extent: vk::Extent2D,

  pub quad_vertices: vk::Buffer,
  pub quad_indices: vk::Buffer,
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
      texture_extent,
      quad_vertices,
      quad_indices,
    })
  }
}

impl DeviceManuallyDestroyed for SpriteBuffers {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.texture.destroy_self(device);
    self.quad_vertices.destroy_self(device);
    self.quad_indices.destroy_self(device);
  }
}
