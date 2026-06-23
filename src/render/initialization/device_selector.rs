use ash::vk;
use vkinitialization::{
  device::{
    device_selector::{
      self, PhysicalDeviceSelection, PhysicalDeviceSelectionError, PhysicalDeviceSelectionSuccess,
    },
    QueueFamilies,
  },
  Surface, SurfaceError,
};

use crate::render::{
  format_conversions::KNOWN_FORMATS,
  pipelines::{ComputePushConstants, GraphicsPushConstants},
  TARGET_API_VERSION,
};

fn supports_swapchain(device: vk::PhysicalDevice, surface: &Surface) -> Result<bool, SurfaceError> {
  let formats = unsafe { surface.get_formats(device) }?;
  let present_modes = unsafe { surface.get_present_modes(device) }?;

  Ok(!formats.is_empty() && !present_modes.is_empty())
}

fn check_physical_device_capabilities(
  instance: &ash::Instance,
  surface: &Surface,
  selection: &PhysicalDeviceSelection,
) -> Result<bool, SurfaceError> {
  // Filter devices that are strictly not supported
  // Check for any features or limits required by the application

  if selection.properties.p10.api_version < TARGET_API_VERSION {
    log::info!(
      "Skipped physical device: Device API version is less than targeted by the application"
    );
    return Ok(false);
  }

  // device supports any of the known formats
  if !KNOWN_FORMATS
    .iter()
    .any(|&f| super::format_is_supported(instance, selection.physical_device, f))
  {
    log::error!("Skipped physical device: Device does not support any known format required by the application");
    return Ok(false);
  }

  if !selection.supported_extensions.swapchain
    || !supports_swapchain(selection.physical_device, surface)?
  {
    log::warn!("Skipped physical device: Device does not support swapchain");
    return Ok(false);
  }

  if selection.supported_features.f13.synchronization2 != vk::TRUE {
    log::warn!("Skipped physical device: Device does not support synchronization features");
    return Ok(false);
  }

  if (selection.properties.p10.limits.max_push_constants_size as usize)
    < size_of::<GraphicsPushConstants>().max(size_of::<ComputePushConstants>())
  {
    log::error!("Skipped physical device: Device does not support required push constant size");
    return Ok(false);
  }

  Ok(true)
}

pub fn select_physical_device<'a>(
  instance: &'a ash::Instance,
  surface: &Surface,
) -> Result<Option<PhysicalDeviceSelectionSuccess<'a>>, PhysicalDeviceSelectionError> {
  let selections = device_selector::enumerate_physical_devices_for_selection(instance)?;
  let mut usable_devices = Vec::with_capacity(selections.len());
  for selection in selections {
    let is_capable = check_physical_device_capabilities(instance, surface, &selection)?;
    if is_capable {
      let queue_families =
        QueueFamilies::get_from_physical_device(instance, selection.physical_device, surface)?;

      usable_devices.push((selection, queue_families));
    }
  }

  let selected_device = usable_devices
    .into_iter()
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
