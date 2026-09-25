use std::ops::BitOr;

use ash::vk;
use vkallocator::{AllocationError, DetailedMemory, MemoryBound};
use vkinitialization::device::{Device, PhysicalDevice};
use vkobjects::{
  destroy, fill_destroyable_array_from_iter, fill_destroyable_array_with_expression,
  utility::OnErr, DeviceManuallyDestroyed,
};

use super::{
  create_objs::{create_color_image_view, create_depth_image_view, create_image},
  FRAMES_IN_FLIGHT, RENDER_EXTENT,
};

// images that the main graphics pipeline draws to
// these are then copied to the swapchain image
#[derive(Debug)]
pub struct RenderTargets {
  pub color_images: [vk::Image; FRAMES_IN_FLIGHT],
  pub depth_images: [vk::Image; FRAMES_IN_FLIGHT],

  pub color_views: [vk::ImageView; FRAMES_IN_FLIGHT],
  pub depth_views: [vk::ImageView; FRAMES_IN_FLIGHT],

  pub memories: Box<[DetailedMemory]>,
}

// todo: add device checking (for writing)
pub const DEPTH_FORMAT: vk::Format = vk::Format::D32_SFLOAT;

impl RenderTargets {
  const PRIORITY: f32 = 0.8; // high priority

  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    render_format: vk::Format,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, AllocationError> {
    let color_images: [vk::Image; FRAMES_IN_FLIGHT] = fill_destroyable_array_with_expression!(
      device,
      create_image(
        device,
        render_format,
        RENDER_EXTENT.width,
        RENDER_EXTENT.height,
        vk::ImageUsageFlags::COLOR_ATTACHMENT
          .bitor(vk::ImageUsageFlags::TRANSFER_SRC)
          .bitor(vk::ImageUsageFlags::TRANSFER_DST),
        #[cfg(feature = "vl")]
        marker,
        #[cfg(feature = "vl")]
        c"Color render target"
      ),
      FRAMES_IN_FLIGHT
    )?;
    let depth_images: [vk::Image; FRAMES_IN_FLIGHT] = fill_destroyable_array_with_expression!(
      device,
      create_image(
        device,
        DEPTH_FORMAT,
        RENDER_EXTENT.width,
        RENDER_EXTENT.height,
        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT
          .bitor(vk::ImageUsageFlags::TRANSFER_SRC)
          .bitor(vk::ImageUsageFlags::TRANSFER_DST),
        #[cfg(feature = "vl")]
        marker,
        #[cfg(feature = "vl")]
        c"Depth render target"
      ),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { destroy!(device => color_images.as_ref()) })?;

    let images_trait = {
      let mut temp = [&color_images[0] as &dyn MemoryBound; FRAMES_IN_FLIGHT * 2];
      for (i, image) in color_images.iter().chain(depth_images.iter()).enumerate() {
        temp[i] = image as &dyn MemoryBound;
      }
      temp
    };

    let alloc = vkallocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
      ],
      images_trait,
      Self::PRIORITY,
      false,
      #[cfg(feature = "log_alloc")]
      None,
      #[cfg(feature = "log_alloc")]
      "MAIN RENDER TARGETS",
    )
    .on_err(|_| unsafe { destroy!(device => color_images.as_ref(), depth_images.as_ref()) })?;

    let color_views = fill_destroyable_array_from_iter!(
      device,
      color_images
        .iter()
        .map(|image| create_color_image_view(device, *image, render_format)),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe {
      destroy!(device => color_images.as_ref(), depth_images.as_ref(), &alloc)
    })?;
    let depth_views = fill_destroyable_array_from_iter!(
      device,
      depth_images
        .iter()
        .map(|image| create_depth_image_view(device, *image, DEPTH_FORMAT)),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe {
      destroy!(device =>color_views.as_ref(), color_images.as_ref(), depth_images.as_ref(), &alloc)
    })?;

    Ok(Self {
      color_images,
      color_views,
      depth_images,
      depth_views,
      memories: Box::from(alloc.get_memories()),
    })
  }
}

impl DeviceManuallyDestroyed for RenderTargets {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.color_views.destroy_self(device);
    self.depth_views.destroy_self(device);

    self.color_images.destroy_self(device);
    self.depth_images.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
