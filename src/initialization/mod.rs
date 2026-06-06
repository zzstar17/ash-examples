use std::{marker::PhantomData, ptr};

use ash::vk;
use vkobjects::const_flag_bitor;

use crate::{APPLICATION_NAME, APPLICATION_VERSION, TARGET_API_VERSION};

mod device_selector;

pub use device_selector::select_physical_device;

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

const REQUIRED_IMAGE_FORMAT_FEATURES: vk::FormatFeatureFlags = const_flag_bitor!(
  vk::FormatFeatureFlags =>
  vk::FormatFeatureFlags::TRANSFER_SRC,
  vk::FormatFeatureFlags::COLOR_ATTACHMENT
);

const REQUIRED_IMAGE_USAGES: vk::ImageUsageFlags = const_flag_bitor!(
  vk::ImageUsageFlags =>
  vk::ImageUsageFlags::TRANSFER_SRC,
  vk::ImageUsageFlags::COLOR_ATTACHMENT
);
