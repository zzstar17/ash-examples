use ash::vk::{self, Handle};
use vkinitialization::device::Device;
use vkobjects::{errors::OutOfMemoryError, DeviceManuallyDestroyed};

use crate::{
  asset_loader::texture_loader::LoadedSkybox,
  render::{
    create_objs::{create_color_cubemap_view, create_image_cubemap},
    gpu_data::{GPUDataAllocationError, TEXTURE_USAGES},
  },
};

#[derive(Debug, Clone, Copy)]
pub struct Skybox {
  pub image: vk::Image,
  pub view: vk::ImageView,
  pub extent: vk::Extent2D,
  pub format: vk::Format,
  pub layer_size: u64,
  pub total_size: u64,
}

impl Skybox {
  pub fn new(
    device: &Device,
    data: &LoadedSkybox,
    texture_format: vk::Format,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let cubemap = create_image_cubemap(
      device,
      texture_format,
      data.width,
      data.height,
      1,
      TEXTURE_USAGES,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Texture",
    )?;
    let data_len = data
      .reader
      .levels()
      .next()
      .unwrap()
      .uncompressed_byte_length;

    Ok(Self {
      image: cubemap,
      view: vk::ImageView::null(),
      extent: vk::Extent2D {
        width: data.width,
        height: data.height,
      },
      format: texture_format,
      layer_size: data_len / 6,
      total_size: data_len,
    })
  }

  pub fn create_view(
    &mut self,
    device: &Device,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<(), OutOfMemoryError> {
    let view = create_color_cubemap_view(
      device,
      self.image,
      self.format,
      1,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Skybox",
    )?;
    self.view = view;
    Ok(())
  }
}

impl DeviceManuallyDestroyed for Skybox {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    if !self.view.is_null() {
      self.view.destroy_self(device);
    }
    self.image.destroy_self(device);
  }
}
