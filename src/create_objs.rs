use std::{ffi::CStr, marker::PhantomData, ptr};

#[cfg(feature = "vl")]
use ash::vk::Handle;
use ash::vk::{self};
use vkobjects::errors::OutOfMemoryError;

use crate::IMAGE_FORMAT;

pub fn create_semaphore(
  device: &ash::Device,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  #[cfg(feature = "vl")] name: &CStr,
) -> Result<vk::Semaphore, OutOfMemoryError> {
  let create_info = vk::SemaphoreCreateInfo::default();
  unsafe {
    let semaphore = device.create_semaphore(&create_info, None)?;
    #[cfg(feature = "vl")]
    marker.set_obj_name(vk::ObjectType::SEMAPHORE, semaphore.as_raw(), name)?;
    Ok(semaphore)
  }
}

pub fn create_fence(
  device: &ash::Device,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  #[cfg(feature = "vl")] name: &CStr,
) -> Result<vk::Fence, OutOfMemoryError> {
  let create_info = vk::FenceCreateInfo::default();
  unsafe {
    let fence = device.create_fence(&create_info, None)?;
    #[cfg(feature = "vl")]
    marker.set_obj_name(vk::ObjectType::FENCE, fence.as_raw(), name)?;
    Ok(fence)
  }
}

pub fn create_buffer(
  device: &ash::Device,
  size: u64,
  usage: vk::BufferUsageFlags,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  #[cfg(feature = "vl")] name: &CStr,
) -> Result<vk::Buffer, OutOfMemoryError> {
  let create_info = vk::BufferCreateInfo {
    s_type: vk::StructureType::BUFFER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::BufferCreateFlags::empty(),
    size,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(),
    _marker: PhantomData,
  };
  unsafe {
    let buffer = device.create_buffer(&create_info, None)?;
    #[cfg(feature = "vl")]
    marker.set_obj_name(vk::ObjectType::BUFFER, buffer.as_raw(), name)?;
    Ok(buffer)
  }
}

pub fn create_image(
  device: &ash::Device,
  width: u32,
  height: u32,
  usage: vk::ImageUsageFlags,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  #[cfg(feature = "vl")] name: &CStr,
) -> Result<vk::Image, OutOfMemoryError> {
  // 1 color layer 2d image
  let create_info = vk::ImageCreateInfo {
    s_type: vk::StructureType::IMAGE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageCreateFlags::empty(),
    image_type: vk::ImageType::TYPE_2D,
    format: IMAGE_FORMAT,
    extent: vk::Extent3D {
      width,
      height,
      depth: 1,
    },
    mip_levels: 1,
    array_layers: 1,
    samples: vk::SampleCountFlags::TYPE_1,
    tiling: vk::ImageTiling::OPTIMAL,
    usage,
    sharing_mode: vk::SharingMode::EXCLUSIVE,
    queue_family_index_count: 0,
    p_queue_family_indices: ptr::null(), // ignored if sharing mode is exclusive
    initial_layout: vk::ImageLayout::UNDEFINED,
    _marker: PhantomData,
  };
  unsafe {
    let image = device.create_image(&create_info, None)?;
    #[cfg(feature = "vl")]
    marker.set_obj_name(vk::ObjectType::IMAGE, image.as_raw(), name)?;
    Ok(image)
  }
}

pub fn create_image_view(
  device: &ash::Device,
  image: vk::Image,
) -> Result<vk::ImageView, OutOfMemoryError> {
  let create_info = vk::ImageViewCreateInfo {
    s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::ImageViewCreateFlags::empty(),
    image,
    view_type: vk::ImageViewType::TYPE_2D,
    format: IMAGE_FORMAT,
    components: vk::ComponentMapping {
      r: vk::ComponentSwizzle::IDENTITY,
      g: vk::ComponentSwizzle::IDENTITY,
      b: vk::ComponentSwizzle::IDENTITY,
      a: vk::ComponentSwizzle::IDENTITY,
    },
    subresource_range: vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    },
    _marker: PhantomData,
  };

  unsafe {
    device
      .create_image_view(&create_info, None)
      .map_err(|err| err.into())
  }
}
