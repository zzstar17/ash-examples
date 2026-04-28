use std::ffi::CStr;

use ash::vk;

use crate::{
  errors::OutOfMemoryError, utility, IMAGE_FORMAT, IMAGE_HEIGHT, IMAGE_MINIMAL_SIZE, IMAGE_WIDTH,
  TARGET_API_VERSION,
};

use super::{
  vendor::Vendor, PhysicalDeviceFeatures, PhysicalDeviceProperties, QueueFamilies,
  REQUIRED_IMAGE_FORMAT_FEATURES, REQUIRED_IMAGE_USAGES,
};

#[derive(Debug, thiserror::Error)]
pub enum DeviceSelectionError {
  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),
  #[error("instance.enumerate_physical_devices() returned VK_ERROR_INITIALIZATION_FAILED")]
  VulkanInitializationFailed,
}

impl From<vk::Result> for DeviceSelectionError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        Self::OutOfMemory(value.into())
      }
      vk::Result::ERROR_INITIALIZATION_FAILED => Self::VulkanInitializationFailed,
      _ => panic!(),
    }
  }
}

fn log_device_properties(properties: &vk::PhysicalDeviceProperties) {
  let vendor = Vendor::from_id(properties.vendor_id);
  let driver_version = vendor.parse_driver_version(properties.driver_version);

  log::info!(
    "\nFound physical device {:?}:
        API Version: {},
        Vendor: {},
        Driver Version: {},
        ID: {},
        Type: {},",
    unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }, // expected to be a valid cstr
    utility::parse_vulkan_api_version(properties.api_version),
    vendor.to_string(),
    driver_version,
    properties.device_id,
    match properties.device_type {
      vk::PhysicalDeviceType::INTEGRATED_GPU => "Integrated GPU",
      vk::PhysicalDeviceType::DISCRETE_GPU => "Discrete GPU",
      vk::PhysicalDeviceType::VIRTUAL_GPU => "Virtual GPU",
      vk::PhysicalDeviceType::CPU => "CPU",
      _ => "Unknown",
    },
  );
}

fn supports_required_image_formats(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> bool {
  let properties =
    unsafe { instance.get_physical_device_format_properties(physical_device, IMAGE_FORMAT) };

  if !properties
    .optimal_tiling_features
    .contains(REQUIRED_IMAGE_FORMAT_FEATURES)
  {
    return false;
  }

  true
}

fn supports_image_dimensions(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
  tiling: vk::ImageTiling,
  usage: vk::ImageUsageFlags,
) -> Result<bool, vk::Result> {
  let properties = unsafe {
    instance.get_physical_device_image_format_properties(
      physical_device,
      IMAGE_FORMAT,
      vk::ImageType::TYPE_2D,
      tiling,
      usage,
      vk::ImageCreateFlags::empty(),
    )?
  };
  log::debug!("image {:?} properties: {:#?}", IMAGE_FORMAT, properties);

  Ok(
    IMAGE_WIDTH <= properties.max_extent.width
      && IMAGE_HEIGHT <= properties.max_extent.height
      && IMAGE_MINIMAL_SIZE <= properties.max_resource_size,
  )
}

pub unsafe fn select_physical_device(
  instance: &ash::Instance,
) -> Result<
  Option<(
    vk::PhysicalDevice,
    PhysicalDeviceProperties<'_>,
    PhysicalDeviceFeatures<'_>,
    QueueFamilies,
  )>,
  DeviceSelectionError,
> {
  Ok(
    instance
      .enumerate_physical_devices()?
      .into_iter()
      .filter_map(|physical_device| {
        // Filter devices that are strictly not supported
        // Check for any features or limits required by the application

        let properties = super::get_extended_properties(instance, physical_device);
        log_device_properties(&properties.p10);
        let features = super::get_extended_features(instance, physical_device);

        if properties.p10.api_version < TARGET_API_VERSION {
          log::info!(
            "Skipped physical device: Device API version is less than targeted by the application"
          );
          return None;
        }

        if !supports_required_image_formats(instance, physical_device) {
          log::warn!("Skipped physical device: Device does not support all required image formats");
          return None;
        }

        match supports_image_dimensions(
          instance,
          physical_device,
          vk::ImageTiling::OPTIMAL,
          REQUIRED_IMAGE_USAGES,
        ) {
          Ok(supports_dimensions) => {
            if !supports_dimensions {
              log::error!("Skipped physical device: Device does not required image dimensions");
              return None;
            }
          }
          Err(err) => {
            log::error!("Device selection error: {:?}", err);
            return None;
          }
        }

        if features.f12.timeline_semaphore != vk::TRUE {
          log::warn!("Skipped physical device: Device does not support timeline semaphores");
          return None;
        }

        if features.f13.synchronization2 != vk::TRUE {
          log::warn!("Skipped physical device: Device does not support synchronization features");
          return None;
        }

        Some((physical_device, properties, features))
      })
      .filter_map(|(physical_device, properties, features)| {
        // filter devices that do not have required queue families
        match QueueFamilies::get_from_physical_device(instance, physical_device) {
          Err(()) => {
            log::info!("Skipped physical device: Device does not contain required queue families");
            None
          }
          Ok(families) => Some((physical_device, properties, features, families)),
        }
      })
      .min_by_key(|(physical_device, _properties, _features, families)| {
        // Assign a score to each device and select the best one available
        // A full application may use multiple metrics like limits, queue families and even the
        // device id to rank each device that a user can have

        let queue_family_importance = 3;
        let device_score_importance = 0;

        // rank devices by number of specialized queue families
        let compute_score = {
          #[cfg(not(feature = "graphics_family"))]
          let score = 0;
          #[cfg(all(feature = "graphics_family", not(feature = "compute_family")))]
          let score = 0;
          #[cfg(all(feature = "graphics_family", feature = "compute_family"))]
          let score = if families.compute.is_some() { 0 } else { 2 };
          score
        };
        let transfer_score = {
          #[cfg(not(feature = "transfer_family"))]
          let score = 0;
          #[cfg(feature = "transfer_family")]
          let score = if families.transfer.is_some() { 0 } else { 1 };
          score
        };
        let queue_score = compute_score + transfer_score;

        // rank devices by commonly most powerful device type
        let device_score = match instance
          .get_physical_device_properties(*physical_device)
          .device_type
        {
          vk::PhysicalDeviceType::DISCRETE_GPU => 0,
          vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
          vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
          vk::PhysicalDeviceType::CPU => 3,
          vk::PhysicalDeviceType::OTHER => 4,
          _ => 5,
        };

        (queue_score << queue_family_importance) + (device_score << device_score_importance)
      }),
  )
}
