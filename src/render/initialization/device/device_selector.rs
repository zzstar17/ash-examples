use std::ffi::CStr;

use ash::vk;

use crate::{
  render::{
    errors::OutOfMemoryError,
    format_conversions::KNOWN_FORMATS,
    initialization::{Surface, SurfaceError},
    RenderPosition, TARGET_API_VERSION,
  },
  utility,
};

use super::{
  queues::QueueFamilyError, vendor::Vendor, EnabledDeviceExtensions, PhysicalDeviceFeatures,
  PhysicalDeviceProperties, QueueFamilies,
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

fn supports_swapchain(device: vk::PhysicalDevice, surface: &Surface) -> Result<bool, SurfaceError> {
  let formats = unsafe { surface.get_formats(device) }?;
  let present_modes = unsafe { surface.get_present_modes(device) }?;

  Ok(!formats.is_empty() && !present_modes.is_empty())
}

fn check_physical_device_capabilities(
  instance: &ash::Instance,
  surface: &Surface,
  physical_device: vk::PhysicalDevice,
  properties: &PhysicalDeviceProperties,
  features: &PhysicalDeviceFeatures,
  supported_extensions: &EnabledDeviceExtensions,
) -> Result<bool, SurfaceError> {
  // Filter devices that are strictly not supported
  // Check for any features or limits required by the application

  if properties.p10.api_version < TARGET_API_VERSION {
    log::info!(
      "Skipped physical device: Device API version is less than targeted by the application"
    );
    return Ok(false);
  }

  // device supports any of the known formats
  if !KNOWN_FORMATS
    .iter()
    .any(|&f| super::format_is_supported(instance, physical_device, f))
  {
    log::error!("Skipped physical device: Device does not support any known format required by the application");
    return Ok(false);
  }

  if !supported_extensions.swapchain || !supports_swapchain(physical_device, surface)? {
    log::warn!("Skipped physical device: Device does not support swapchain");
    return Ok(false);
  }

  if features.f13.synchronization2 != vk::TRUE {
    log::warn!("Skipped physical device: Device does not support synchronization features");
    return Ok(false);
  }

  if (properties.p10.limits.max_push_constants_size as usize) < size_of::<RenderPosition>() {
    log::warn!("Skipped physical device: Device does not support required push constant size");
    return Ok(false);
  }

  Ok(true)
}

pub unsafe fn select_physical_device<'a>(
  instance: &'a ash::Instance,
  surface: &'a Surface,
) -> Result<
  Option<(
    vk::PhysicalDevice,
    PhysicalDeviceProperties<'a>,
    PhysicalDeviceFeatures<'a>,
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
        let supported_extensions = match EnabledDeviceExtensions::mark_supported_by_physical_device(
          instance,
          physical_device,
        ) {
          Ok(v) => v,
          Err(err) => {
            log::error!("Device selection error: {:?}", err);
            return None;
          }
        };

        match check_physical_device_capabilities(
          instance,
          surface,
          physical_device,
          &properties,
          &features,
          &supported_extensions,
        ) {
          Ok(all_good) => {
            if all_good {
              Some((physical_device, properties, features))
            } else {
              None
            }
          }
          Err(err) => {
            log::error!("Device selection error: {:?}", err);
            None
          }
        }
      })
      .filter_map(|(physical_device, properties, features)| {
        // filter devices that do not have required queue families
        match QueueFamilies::get_from_physical_device(instance, physical_device, surface) {
          Err(err) => {
            match err {
              QueueFamilyError::DoesNotSupportRequiredQueueFamilies => log::info!(
                "Skipped physical device: Device does not contain required queue families"
              ),
              QueueFamilyError::SurfaceError(err) => log::error!(
                "Device selection error during queue family retrieval: {:?}",
                err
              ),
            }
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
        let queue_score = if families.transfer.is_some() { 0 } else { 1 };

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
