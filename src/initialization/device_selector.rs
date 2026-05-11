use ash::vk;
use vkinitialization::device::{
  device_selector::{self, PhysicalDeviceSelectionError, PhysicalDeviceSelectionSuccess},
  QueueFamilies,
};

use crate::{
  initialization::REQUIRED_IMAGE_USAGES, IMAGE_FORMAT, IMAGE_HEIGHT, IMAGE_MINIMAL_SIZE,
  IMAGE_WIDTH, TARGET_API_VERSION,
};

fn supports_required_image_formats(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> bool {
  let properties =
    unsafe { instance.get_physical_device_format_properties(physical_device, IMAGE_FORMAT) };

  if !properties
    .optimal_tiling_features
    .contains(super::REQUIRED_IMAGE_FORMAT_FEATURES)
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

pub fn select_physical_device<'a>(
  instance: &'a ash::Instance,
) -> Result<Option<PhysicalDeviceSelectionSuccess<'a>>, PhysicalDeviceSelectionError> {
  let devices = device_selector::enumerate_physical_devices_for_selection(instance)?;
  let selected_device = devices
    .into_iter()
    .filter(|selection| {
      // Filter devices that are strictly not supported
      // Check for any features or limits required by the application

      if selection.properties.p10.api_version < TARGET_API_VERSION {
        log::warn!(
          "Skipped physical device: Device API version is less than targeted by the application"
        );
        return false;
      }

      if !supports_required_image_formats(instance, selection.physical_device) {
        log::warn!("Skipped physical device: Device does not support all required image formats");
        return false;
      }

      match supports_image_dimensions(
        instance,
        selection.physical_device,
        vk::ImageTiling::OPTIMAL,
        REQUIRED_IMAGE_USAGES,
      ) {
        Ok(supports_dimensions) => {
          if !supports_dimensions {
            log::error!("Skipped physical device: Device does not required image dimensions");
            return false;
          }
        }
        Err(err) => {
          log::error!("Device selection error: {:?}", err);
          return false;
        }
      }

      if selection.supported_features.f13.synchronization2 != vk::TRUE {
        log::warn!("Skipped physical device: Device does not support synchronization features");
        return false;
      }

      true
    })
    .filter_map(|selection| {
      // filter devices that do not have required queue families
      match QueueFamilies::get_from_physical_device(instance, selection.physical_device) {
        Err(err) => {
          log::warn!("Skipped physical device: {}", err);
          None
        }
        Ok(families) => Some((selection, families)),
      }
    })
    .min_by_key(|(selection, families)| {
      // Assign a score to each device and select the best one available
      // A full application may use multiple metrics like limits, queue families and even the
      // device id to rank each device that a user can have

      let queue_family_importance = 3;
      let device_score_importance = 0;

      // rank devices by number of specialized queue families
      let transfer_score = if families.transfer.is_some() { 0 } else { 1 };
      let queue_score = transfer_score;

      // rank devices by commonly most powerful device type
      let device_score = match selection.properties.p10.device_type {
        vk::PhysicalDeviceType::DISCRETE_GPU => 0,
        vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
        vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
        vk::PhysicalDeviceType::CPU => 3,
        vk::PhysicalDeviceType::OTHER => 4,
        _ => 5,
      };

      (queue_score << queue_family_importance) + (device_score << device_score_importance)
    });

  Ok(selected_device.map(
    |(selection, queue_families)| PhysicalDeviceSelectionSuccess {
      physical_device: selection.physical_device,
      properties: selection.properties,
      supported_extensions: selection.supported_extensions,
      supported_features: selection.supported_features,
      queue_families,
    },
  ))
}
