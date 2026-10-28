use std::ops::BitOr;

use ash::vk::{self, Handle};
use vkinitialization::device::Device;
use vkobjects::{destroy, utility::OnErr, DeviceManuallyDestroyed};

use crate::{
  asset_loader::model_loader::ModelGpuData,
  render::{
    create_objs::{create_buffer, create_image},
    gpu_data::{GPUDataAllocationError, TEXTURE_USAGES},
  },
};

#[derive(Debug, Clone, Copy)]
pub struct Texture {
  pub image: vk::Image,
  pub view: vk::ImageView,
  pub mip_levels: u32,
  pub extent: vk::Extent2D,
  pub format: vk::Format,
  pub total_size: u64,
}

#[derive(Debug, Clone)]
pub struct SpriteBuffers {
  pub textures: Vec<Texture>,

  pub vertices: vk::Buffer,
  pub indices: vk::Buffer,
}

impl SpriteBuffers {
  pub fn new(
    device: &Device,
    models: &ModelGpuData,
    texture_format: vk::Format,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let mut textures = Vec::with_capacity(models.images.len());
    for image_data in models.images.iter() {
      let mip_levels = image_data.reader.header().level_count.max(1);
      let image = create_image(
        device,
        texture_format,
        image_data.width,
        image_data.height,
        mip_levels,
        TEXTURE_USAGES,
        #[cfg(feature = "vl")]
        marker,
        #[cfg(feature = "vl")]
        c"Texture",
      )
      .on_err(|_| unsafe { textures.destroy_self(device) })?;
      textures.push(Texture {
        image,
        view: vk::ImageView::null(),
        mip_levels,
        extent: vk::Extent2D {
          width: image_data.width,
          height: image_data.height,
        },
        format: texture_format,
        total_size: image_data.total_mip_levels_size,
      });
    }

    let vertices: vk::Buffer = create_buffer(
      device,
      models.vertices_size(),
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Vertex buffer",
    )
    .on_err(|_| unsafe { textures.destroy_self(device) })?;
    let indices: vk::Buffer = create_buffer(
      device,
      models.indices_size(),
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Index buffer",
    )
    .on_err(|_| unsafe { destroy!(device => &vertices, &textures.as_ref()) })?;

    Ok(Self {
      textures,
      vertices,
      indices,
    })
  }
}

impl DeviceManuallyDestroyed for SpriteBuffers {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.textures.destroy_self(device);
    self.vertices.destroy_self(device);
    self.indices.destroy_self(device);
  }
}

impl DeviceManuallyDestroyed for Texture {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.view.destroy_self(device);
    if !self.image.is_null() {
      self.image.destroy_self(device);
    }
  }
}
