use std::{ffi::CStr, ops::Deref};

use ash::vk;

use crate::render::{allocator, initialization::Surface};

use super::{device_selector::DeviceSelectionError, select_physical_device};

use super::QueueFamilies;

pub struct CustomProperties {
  // p10
  pub driver_version: u32,
  pub vendor_id: u32,
  pub device_id: u32,
  pub pipeline_cache_uuid: [u8; vk::UUID_SIZE],

  // p11
  pub max_memory_allocation_size: u64,
}

// Saves physical device additional information in order to not query it multiple times
pub struct PhysicalDevice {
  inner: vk::PhysicalDevice,
  pub queue_families: QueueFamilies,
  pub mem_properties: vk::PhysicalDeviceMemoryProperties,
  pub properties: CustomProperties,
  pub queue_family_properties: Box<[vk::QueueFamilyProperties]>,
}

impl Deref for PhysicalDevice {
  type Target = vk::PhysicalDevice;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl PhysicalDevice {
  pub unsafe fn select(
    instance: &ash::Instance,
    surface: &Surface,
  ) -> Result<Option<PhysicalDevice>, DeviceSelectionError> {
    match select_physical_device(instance, surface)? {
      Some((physical_device, properties, _features, queue_families)) => {
        let mem_properties = instance.get_physical_device_memory_properties(physical_device);
        let queue_family_properties = instance
          .get_physical_device_queue_family_properties(physical_device)
          .into_boxed_slice();

        log::info!(
          "Using physical device {:?}",
          unsafe { CStr::from_ptr(properties.p10.device_name.as_ptr()) }, // expected to be a valid cstr
        );
        print_queue_families_debug_info(&queue_family_properties);
        #[cfg(feature = "log_alloc")]
        allocator::debug_print_device_memory_info(&mem_properties).unwrap();

        Ok(Some(PhysicalDevice {
          inner: physical_device,
          queue_families,
          mem_properties,
          properties: CustomProperties {
            driver_version: properties.p10.driver_version,
            vendor_id: properties.p10.vendor_id,
            device_id: properties.p10.device_id,
            pipeline_cache_uuid: properties.p10.pipeline_cache_uuid,

            max_memory_allocation_size: properties.p11.max_memory_allocation_size,
          },
          queue_family_properties,
        }))
      }
      None => Ok(None),
    }
  }

  pub fn memory_types(&self) -> &[vk::MemoryType] {
    &self.mem_properties.memory_types[0..(self.mem_properties.memory_type_count as usize)]
  }
}

fn print_queue_families_debug_info(properties: &[vk::QueueFamilyProperties]) {
  log::debug!("Physical device queue family properties: {:#?}", properties);
}
