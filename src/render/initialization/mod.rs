mod device_selector;
mod pre_window_init;

use ash::vk;
pub use device_selector::select_physical_device;

use std::{marker::PhantomData, ptr};

pub use pre_window_init::{RenderInit, RenderInitError};

use crate::{
  render::{gpu_data::TEXTURE_FORMAT_FEATURES, TARGET_API_VERSION},
  APPLICATION_NAME, APPLICATION_VERSION,
};

pub fn get_app_info<'a>() -> vk::ApplicationInfo<'a> {
  vk::ApplicationInfo {
    s_type: vk::StructureType::APPLICATION_INFO,
    api_version: TARGET_API_VERSION,
    p_application_name: APPLICATION_NAME.as_ptr(),
    application_version: APPLICATION_VERSION,
    p_engine_name: ptr::null(),
    engine_version: vk::make_api_version(0, 1, 0, 0),
    p_next: ptr::null(),
    _marker: PhantomData,
  }
}

pub fn format_is_supported(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
  format: vk::Format,
) -> bool {
  let properties =
    unsafe { instance.get_physical_device_format_properties(physical_device, format) };

  properties
    .optimal_tiling_features
    .contains(TEXTURE_FORMAT_FEATURES)
}
