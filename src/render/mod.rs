mod allocator;
mod command_pools;
mod create_objs;
mod data;
mod descriptor_sets;
mod device_destroyable;
mod errors;
mod initialization;
mod pipelines;
mod render_object;
mod render_pass;
mod renderer;
mod shaders;
mod swapchain;
mod vertices;

use crate::cstr;
use ash::vk;
use std::ffi::CStr;

pub use initialization::RenderInit;

const FRAMES_IN_FLIGHT: usize = 2;

// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
#[cfg(feature = "vl")]
const VALIDATION_LAYERS: [&'static CStr; 1] = [cstr!("VK_LAYER_KHRONOS_validation")];
#[cfg(feature = "vl")]
const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];

const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

const REQUIRED_DEVICE_EXTENSIONS: [&'static CStr; 1] = [cstr!("VK_KHR_swapchain")];
