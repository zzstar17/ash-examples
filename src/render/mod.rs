mod command_pools;
mod create_objs;
mod descriptor_sets;
mod errors;
mod format_conversions;
mod gpu_data;
mod initialization;
mod pipelines;
mod render_object;
mod render_pass;
mod render_targets;
mod renderer;
mod screenshot_buffer;
mod shaders;
mod swapchain;
mod sync_renderer;
mod vertices;

use ash::vk;
use vkobjects::const_flag_bitor;

pub use errors::{FrameRenderError, InitializationError};
pub use initialization::{RenderInit, RenderInitError};
pub use render_object::RenderPosition;
pub use swapchain::AcquireNextImageError;
pub use sync_renderer::SyncRenderer;

use crate::RESOLUTION;

const FRAMES_IN_FLIGHT: usize = 2;

const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

const SWAPCHAIN_IMAGE_USAGES: vk::ImageUsageFlags = const_flag_bitor!(vk::ImageUsageFlags => vk::ImageUsageFlags::COLOR_ATTACHMENT, vk::ImageUsageFlags::TRANSFER_DST);

pub const RENDER_EXTENT: vk::Extent2D = vk::Extent2D {
  width: RESOLUTION[0],
  height: RESOLUTION[1],
};

// minimum memory size of an image that can be rendered to with the specified resolution
const IMAGE_WITH_RESOLUTION_MINIMAL_SIZE: u64 =
  RENDER_EXTENT.width as u64 * RENDER_EXTENT.height as u64 * 4;

// https://stackoverflow.com/questions/66401081/vulkan-swapchain-format-unorm-vs-srgb
// https://stackoverflow.com/questions/75094730/why-prefer-non-srgb-format-for-vulkan-swapchain
// we're using the same format for the render target and the swapchain, so there is no
// difference in color for Ferris, only for the background color (as the color gets interpreted differently)
const SWAPCHAIN_PREFERRED_IMAGE_FORMAT: vk::Format = vk::Format::B8G8R8A8_UNORM;
