use std::ops::BitOr;

use ash::vk;
use vkallocator::{AllocationError, MemoryBound, MemoryWithType};
use vkinitialization::device::{Device, PhysicalDevice};
use vkobjects::{
  destroy, fill_destroyable_array_from_iter, fill_destroyable_array_with_expression,
  utility::OnErr, DeviceManuallyDestroyed,
};

use super::{
  create_objs::{create_image, create_image_view},
  render_pass::create_framebuffer,
  FRAMES_IN_FLIGHT, RENDER_EXTENT,
};

// images that the main graphics pipeline draws to
// these are then copied to the swapchain image
#[derive(Debug)]
pub struct RenderTargets {
  pub images: [vk::Image; FRAMES_IN_FLIGHT],
  pub memories: Box<[MemoryWithType]>,
  pub image_views: [vk::ImageView; FRAMES_IN_FLIGHT],
  pub framebuffers: [vk::Framebuffer; FRAMES_IN_FLIGHT],
}

impl RenderTargets {
  const PRIORITY: f32 = 0.8; // high priority

  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    render_pass: vk::RenderPass,
    render_format: vk::Format,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, AllocationError> {
    let images: [vk::Image; FRAMES_IN_FLIGHT] = fill_destroyable_array_with_expression!(
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
        c"Frames"
      ),
      FRAMES_IN_FLIGHT
    )?;

    let images_trait = {
      let mut temp = [&images[0] as &dyn MemoryBound; FRAMES_IN_FLIGHT];
      for i in 0..FRAMES_IN_FLIGHT {
        temp[i] = &images[i] as &dyn MemoryBound;
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
      #[cfg(feature = "log_alloc")]
      None,
      #[cfg(feature = "log_alloc")]
      "MAIN RENDER TARGETS",
    )?;

    let image_views = fill_destroyable_array_from_iter!(
      device,
      images
        .iter()
        .map(|image| create_image_view(device, *image, render_format)),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { destroy!(device => images.as_ref(), &alloc) })?;

    let framebuffers = fill_destroyable_array_from_iter!(
      device,
      image_views
        .iter()
        .map(|view| create_framebuffer(device, render_pass, *view, RENDER_EXTENT)),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { destroy!(device => image_views.as_ref(), images.as_ref(), &alloc) })?;

    Ok(Self {
      images,
      image_views,
      memories: Box::from(alloc.get_memories()),
      framebuffers,
    })
  }
}

impl DeviceManuallyDestroyed for RenderTargets {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.framebuffers.destroy_self(device);
    self.image_views.destroy_self(device);
    self.images.destroy_self(device);
    self.memories.destroy_self(device);
  }
}
