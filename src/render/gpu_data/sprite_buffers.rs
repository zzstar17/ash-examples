use std::ops::BitOr;

use ash::vk;
use vkinitialization::device::Device;
use vkobjects::{destroy, utility::OnErr, DeviceManuallyDestroyed};

use crate::{
  asset_loader::{
    texture_loader::{self, TextureOffsets},
    LoadedModels, Models,
  },
  render::{
    create_objs::{create_buffer, create_image},
    gpu_data::{GPUDataAllocationError, TEXTURE_USAGES},
  },
};

#[derive(Debug, Clone, Copy)]
pub struct SpriteBuffers {
  pub texture: vk::Image,
  pub texture_extent: vk::Extent2D,
  pub texture_offsets: TextureOffsets,

  pub vertices: vk::Buffer,
  pub indices: vk::Buffer,
  pub models: Models,
}

impl SpriteBuffers {
  pub fn new(
    device: &Device,
    loaded_models: &LoadedModels,
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

    let vertices: vk::Buffer = create_buffer(
      device,
      loaded_models.vertices_size(),
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Vertex buffer",
    )
    .on_err(|_| unsafe { texture.destroy_self(device) })?;
    let indices: vk::Buffer = create_buffer(
      device,
      loaded_models.indices_size(),
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Index buffer",
    )
    .on_err(|_| unsafe { destroy!(device => &vertices, &texture) })?;

    Ok(Self {
      texture,
      texture_extent,
      vertices,
      indices,
      models: loaded_models.models,
      texture_offsets: texture_loader::get_texture_offsets(),
    })
  }
}

impl DeviceManuallyDestroyed for SpriteBuffers {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.texture.destroy_self(device);
    self.vertices.destroy_self(device);
    self.indices.destroy_self(device);
  }
}
