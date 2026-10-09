use std::ops::BitOr;

use ash::vk;
use vkallocator::{AllocationError, DetailedMemory, MemoryBound};
use vkinitialization::device::{Device, PhysicalDevice};
use vkobjects::{
  destroy, fill_destroyable_array_from_iter, fill_destroyable_array_with_expression,
  utility::OnErr, DeviceManuallyDestroyed,
};

use crate::render::create_objs::{create_image, create_image_sampled};

use super::{
  create_objs::{create_color_image_view, create_depth_image_view},
  FRAMES_IN_FLIGHT, RENDER_EXTENT,
};

// images that the main graphics pipeline draws to
// these are then copied to the swapchain image
#[derive(Debug)]
pub struct RenderTargets {
  // none if multisampling is 1 (disabled)
  // multisampled color image
  pub color_images: Option<[vk::Image; FRAMES_IN_FLIGHT]>,
  pub depth_images: [vk::Image; FRAMES_IN_FLIGHT],
  // resolve color images with 1 sample
  // directly rendered to if multisampling is disabled
  // used after rendering
  pub resolve_images: [vk::Image; FRAMES_IN_FLIGHT],

  pub color_views: Option<[vk::ImageView; FRAMES_IN_FLIGHT]>,
  pub depth_views: [vk::ImageView; FRAMES_IN_FLIGHT],
  pub resolve_views: [vk::ImageView; FRAMES_IN_FLIGHT],

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
    sample_count: usize,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, AllocationError> {
    let color_images: Option<[vk::Image; FRAMES_IN_FLIGHT]> = if sample_count > 1 {
      let images = fill_destroyable_array_with_expression!(
        device,
        create_image_sampled(
          device,
          render_format,
          RENDER_EXTENT.width,
          RENDER_EXTENT.height,
          sample_count,
          vk::ImageUsageFlags::COLOR_ATTACHMENT,
          #[cfg(feature = "vl")]
          marker,
          #[cfg(feature = "vl")]
          c"Color render target"
        ),
        FRAMES_IN_FLIGHT
      )?;
      Some(images)
    } else {
      None
    };
    let destroy_color_images = || unsafe {
      if let Some(images) = color_images {
        images.destroy_self(device);
      }
    };
    let depth_images: [vk::Image; FRAMES_IN_FLIGHT] = fill_destroyable_array_with_expression!(
      device,
      create_image_sampled(
        device,
        DEPTH_FORMAT,
        RENDER_EXTENT.width,
        RENDER_EXTENT.height,
        sample_count,
        vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
        #[cfg(feature = "vl")]
        marker,
        #[cfg(feature = "vl")]
        c"Depth render target"
      ),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| {
      destroy_color_images();
    })?;

    let resolve_images: [vk::Image; FRAMES_IN_FLIGHT] = fill_destroyable_array_with_expression!(
      device,
      create_image(
        device,
        render_format,
        RENDER_EXTENT.width,
        RENDER_EXTENT.height,
        1,
        vk::ImageUsageFlags::COLOR_ATTACHMENT
          .bitor(vk::ImageUsageFlags::TRANSFER_SRC)
          .bitor(vk::ImageUsageFlags::TRANSFER_DST),
        #[cfg(feature = "vl")]
        marker,
        #[cfg(feature = "vl")]
        c"Resolve render target"
      ),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe {
      destroy!(device => depth_images.as_ref());
      destroy_color_images();
    })?;

    let memories: Box<[DetailedMemory]> = if let Some(color_images) = color_images {
      let images_trait = {
        let mut temp = [&resolve_images[0] as &dyn MemoryBound; FRAMES_IN_FLIGHT * 3];
        for (i, image) in color_images
          .iter()
          .chain(depth_images.iter())
          .chain(resolve_images.iter())
          .enumerate()
        {
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
        Some([
          "Color image 0",
          "Color image 1",
          "Depth image 0",
          "Depth image 1",
          "Resolve image 0",
          "Resolve image 1",
        ]),
        #[cfg(feature = "log_alloc")]
        "MAIN RENDER TARGETS",
      )
      .on_err(|_| unsafe {
        destroy!(device => color_images.as_ref(), depth_images.as_ref(), resolve_images.as_ref())
      })?;
      Box::from(alloc.get_memories())
    } else {
      let images_trait = {
        let mut temp = [&resolve_images[0] as &dyn MemoryBound; FRAMES_IN_FLIGHT * 2];
        for (i, image) in resolve_images.iter().chain(depth_images.iter()).enumerate() {
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
        Some([
          "Resolve image 0",
          "Resolve image 1",
          "Depth image 0",
          "Depth image 1",
        ]),
        #[cfg(feature = "log_alloc")]
        "MAIN RENDER TARGETS",
      )
      .on_err(|_| unsafe { destroy!(device => &resolve_images.as_ref(), depth_images.as_ref()) })?;
      Box::from(alloc.get_memories())
    };

    let destroy_objs = || unsafe {
      if let Some(images) = color_images {
        images.destroy_self(device);
      }
      depth_images.destroy_self(device);
      resolve_images.destroy_self(device);

      memories.destroy_self(device);
    };

    let resolve_views = fill_destroyable_array_from_iter!(
      device,
      resolve_images
        .iter()
        .map(|image| create_color_image_view(device, *image, render_format, 1)),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| {
      destroy_objs();
    })?;
    let depth_views = fill_destroyable_array_from_iter!(
      device,
      depth_images
        .iter()
        .map(|image| create_depth_image_view(device, *image, DEPTH_FORMAT)),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe {
      resolve_views.destroy_self(device);
      destroy_objs();
    })?;

    let color_views = if let Some(images) = color_images {
      let views = fill_destroyable_array_from_iter!(
        device,
        images
          .iter()
          .map(|image| create_color_image_view(device, *image, render_format, 1)),
        FRAMES_IN_FLIGHT
      )
      .on_err(|_| unsafe {
        resolve_views.destroy_self(device);
        depth_views.destroy_self(device);
        destroy_objs();
      })?;
      Some(views)
    } else {
      None
    };

    Ok(Self {
      color_images,
      color_views,
      depth_images,
      depth_views,
      resolve_images,
      resolve_views,
      memories,
    })
  }
}

impl DeviceManuallyDestroyed for RenderTargets {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    if let Some(views) = self.color_views {
      views.destroy_self(device);
    }
    self.depth_views.destroy_self(device);
    self.resolve_views.destroy_self(device);

    if let Some(images) = self.color_images {
      images.destroy_self(device);
    }
    self.depth_images.destroy_self(device);
    self.resolve_images.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
