mod device_selector;
mod logical_device;
mod physical_device;
mod queues;
mod vendor;

pub use device_selector::DeviceSelectionError;
pub use logical_device::{Device, DeviceCreationError};
pub use physical_device::PhysicalDevice;
pub use queues::{QueueFamilies, SingleQueues};

use std::{
  ffi::{CStr}, marker::PhantomData, ptr::{self}
};

use ash::vk;
use device_selector::select_physical_device;

use crate::{
  APPLICATION_NAME, APPLICATION_VERSION, TARGET_API_VERSION, utility::{const_flag_bitor}
};

#[cfg(feature = "graphics_family")]
pub const GRAPHICS_QUEUE_LABEL: &CStr = c"GRAPHICS QUEUE";
#[cfg(feature = "compute_family")]
pub const COMPUTE_QUEUE_LABEL: &CStr = c"COMPUTE QUEUE";
#[cfg(feature = "transfer_family")]
pub const TRANSFER_QUEUE_LABEL: &CStr = c"TRANSFER QUEUE";

const REQUIRED_IMAGE_FORMAT_FEATURES: vk::FormatFeatureFlags = const_flag_bitor!(
  vk::FormatFeatureFlags,
  vk::FormatFeatureFlags::TRANSFER_SRC,
  vk::FormatFeatureFlags::COLOR_ATTACHMENT
);

const REQUIRED_IMAGE_USAGES: vk::ImageUsageFlags = const_flag_bitor!(
  vk::ImageUsageFlags,
  vk::ImageUsageFlags::TRANSFER_SRC,
  vk::ImageUsageFlags::COLOR_ATTACHMENT
);

