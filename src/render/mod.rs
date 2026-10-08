mod command_pools;
mod create_objs;
mod descriptor_sets;
mod errors;
mod format_conversions;
mod gpu_data;
mod initialization;
mod pipelines;
mod render_targets;
mod renderer;
mod screenshot_buffer;
mod shaders;
mod swapchain;
mod sync_renderer;
mod vertices;

use ash::vk;
use cgmath::Vector2;
use vkinitialization::device::PhysicalDevice;
use vkobjects::const_flag_bitor;

pub use errors::{FrameRenderError, InitializationError};
pub use initialization::{RenderInit, RenderInitError};
pub use pipelines::GraphicsPushConstants;
pub use swapchain::AcquireNextImageError;
pub use sync_renderer::SyncRenderer;
pub use vertices::TexturedVertex;

use crate::{MAX_MULTISAMPLE_COUNT, RESOLUTION};

const FRAMES_IN_FLIGHT: usize = 2;

const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

const SWAPCHAIN_IMAGE_USAGES: vk::ImageUsageFlags = const_flag_bitor!(vk::ImageUsageFlags => vk::ImageUsageFlags::COLOR_ATTACHMENT, vk::ImageUsageFlags::TRANSFER_DST);

const RENDER_EXTENT: vk::Extent2D = vk::Extent2D {
  width: RESOLUTION[0],
  height: RESOLUTION[1],
};

const RENDER_SIZE: Vector2<f32> =
  Vector2::new(RENDER_EXTENT.width as f32, RENDER_EXTENT.height as f32);

// minimum memory size of an image that can be rendered to with the specified resolution
const IMAGE_WITH_RESOLUTION_MINIMAL_SIZE: u64 =
  RENDER_EXTENT.width as u64 * RENDER_EXTENT.height as u64 * 4;

// https://stackoverflow.com/questions/66401081/vulkan-swapchain-format-unorm-vs-srgb
// https://stackoverflow.com/questions/75094730/why-prefer-non-srgb-format-for-vulkan-swapchain
// https://vulkan.gpuinfo.org/listsurfaceformats.php
// surely the swapchain supports one of these
const SWAPCHAIN_SUPPORTED_IMAGE_FORMATS: [vk::Format; 4] = [
  vk::Format::R8G8B8A8_SRGB,
  vk::Format::B8G8R8A8_SRGB,
  vk::Format::R8G8B8A8_UNORM,
  vk::Format::B8G8R8A8_UNORM,
];

fn get_multisample_count(physical_device: &PhysicalDevice) -> usize {
  let limit = physical_device
    .properties
    .p10
    .limits
    .framebuffer_color_sample_counts
    & physical_device
      .properties
      .p10
      .limits
      .framebuffer_depth_sample_counts;
  let limit = sample_flags_to_count(limit);

  let user_set = sample_flags_to_count(MAX_MULTISAMPLE_COUNT);

  limit.min(user_set)
}

fn sample_count_to_flags(count: usize) -> vk::SampleCountFlags {
  match count {
    64 => vk::SampleCountFlags::TYPE_64,
    32 => vk::SampleCountFlags::TYPE_32,
    16 => vk::SampleCountFlags::TYPE_16,
    8 => vk::SampleCountFlags::TYPE_8,
    4 => vk::SampleCountFlags::TYPE_4,
    2 => vk::SampleCountFlags::TYPE_2,
    1 => vk::SampleCountFlags::TYPE_1,
    _ => panic!("Invalid sample count ({})", count),
  }
}

fn sample_flags_to_count(flags: vk::SampleCountFlags) -> usize {
  if flags.contains(vk::SampleCountFlags::TYPE_64) {
    return 64;
  }
  if flags.contains(vk::SampleCountFlags::TYPE_32) {
    return 32;
  }
  if flags.contains(vk::SampleCountFlags::TYPE_16) {
    return 16;
  }
  if flags.contains(vk::SampleCountFlags::TYPE_8) {
    return 8;
  }
  if flags.contains(vk::SampleCountFlags::TYPE_4) {
    return 4;
  }
  if flags.contains(vk::SampleCountFlags::TYPE_2) {
    return 2;
  }

  1
}
