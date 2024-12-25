use ash::vk::{self};
use ash_destructor::{DeviceDestroyable, SelfDestroyable};
use std::{
  marker::PhantomData,
  ops::Deref,
  os::raw::c_void,
  ptr::{self},
};

use super::{EnabledDeviceExtensions, PhysicalDevice, Queues};

#[derive(DeviceDestroyable)]
pub struct Device {
  pub inner: ash::Device,
  #[destroy_ignore_remaining]
  pub enabled_extensions: EnabledDeviceExtensions,
  // contains ash's extension loader if pageable_device_local_memory is enabled
  pub pageable_device_local_memory_loader: Option<ash::ext::pageable_device_local_memory::Device>,
}

impl SelfDestroyable for Device {
  unsafe fn destroy_self_alloc(&self, allocation_callbacks: Option<&vk::AllocationCallbacks>) {
    SelfDestroyable::destroy_self_alloc(&self.inner, allocation_callbacks);
  }
}

impl Deref for Device {
  type Target = ash::Device;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl Device {
  pub fn create(
    instance: &ash::Instance,
    physical_device: &PhysicalDevice,
  ) -> Result<(Self, Queues), vk::Result> {
    let queue_create_infos = Queues::get_queue_create_infos(&physical_device.queue_families);

    let to_enable_extensions =
      EnabledDeviceExtensions::mark_supported_by_physical_device(instance, **physical_device)?;
    let extension_ptrs = to_enable_extensions.get_extension_list();

    log::debug!(
      "Enabling the following device extensions:\n{:?}",
      to_enable_extensions
    );

    // enabled features
    let features10 = vk::PhysicalDeviceFeatures::default();
    let mut features12 = vk::PhysicalDeviceVulkan12Features {
      timeline_semaphore: vk::TRUE,
      ..Default::default()
    };
    let mut features13 = vk::PhysicalDeviceVulkan13Features {
      synchronization2: vk::TRUE,
      ..Default::default()
    };

    features12.p_next = &mut features13 as *mut vk::PhysicalDeviceVulkan13Features as *mut c_void;
    features13.p_next = ptr::null_mut();

    #[allow(deprecated)]
    let create_info = vk::DeviceCreateInfo {
      s_type: vk::StructureType::DEVICE_CREATE_INFO,
      p_queue_create_infos: queue_create_infos.as_ptr(),
      queue_create_info_count: queue_create_infos.len() as u32,
      p_enabled_features: &features10,
      p_next: &features12 as *const vk::PhysicalDeviceVulkan12Features as *const c_void,
      pp_enabled_layer_names: ptr::null(), // deprecated
      enabled_layer_count: 0,              // deprecated
      pp_enabled_extension_names: extension_ptrs.as_ptr(),
      enabled_extension_count: extension_ptrs.len() as u32,
      flags: vk::DeviceCreateFlags::empty(),
      _marker: PhantomData,
    };
    log::debug!("Creating logical device");
    let device: ash::Device =
      unsafe { instance.create_device(**physical_device, &create_info, None)? };

    log::debug!("Retrieving queues");
    let queues = unsafe { Queues::retrieve(&device, &physical_device.queue_families) };

    let pageable_device_local_memory_loader = if to_enable_extensions.pageable_device_local_memory {
      Some(ash::ext::pageable_device_local_memory::Device::new(
        instance, &device,
      ))
    } else {
      None
    };

    Ok((
      Self {
        inner: device,
        enabled_extensions: to_enable_extensions,
        pageable_device_local_memory_loader,
      },
      queues,
    ))
  }
}
