use ash::vk::{self};
use std::{
  fmt::Write,
  marker::PhantomData,
  ops::Deref,
  os::raw::c_void,
  ptr::{self},
};

use crate::render::{
  device_destroyable::ManuallyDestroyed, errors::OutOfMemoryError,
  initialization::device::queues::Queue,
};

use super::{EnabledDeviceExtensions, PhysicalDevice, SingleQueues};

pub struct Device {
  pub inner: ash::Device,
  pub enabled_extensions: EnabledDeviceExtensions,
  // contains ash's extension loader if pageable_device_local_memory is enabled
  pub pageable_device_local_memory_loader: Option<ash::ext::pageable_device_local_memory::Device>,
}

impl Deref for Device {
  type Target = ash::Device;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

#[derive(Debug, thiserror::Error)]
pub enum DeviceCreationError {
  #[error("Device creation returned VK_ERROR_INITIALIZATION_FAILED")]
  VulkanInitializationFailed,
  #[error(
    "Device creation returned VK_ERROR_DEVICE_LOST.\
This may indicate problems with the graphics driver or instability with the physical device"
  )]
  DeviceLost,
  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),
}

impl Device {
  pub fn create(
    instance: &ash::Instance,
    physical_device: &PhysicalDevice,
  ) -> Result<(Self, SingleQueues), DeviceCreationError> {
    let (queue_create_infos, unique_queue_size) =
      super::queues::get_single_queue_create_infos(&physical_device.queue_families);

    let to_enable_extensions =
      EnabledDeviceExtensions::mark_supported_by_physical_device(instance, **physical_device)?;
    let extension_ptrs = to_enable_extensions.get_extension_list();

    log::info!(
      "Enabling the following device extensions:\n{:#?}",
      to_enable_extensions
    );

    // enabled features
    let features10 = vk::PhysicalDeviceFeatures::default();
    let mut features12 = vk::PhysicalDeviceVulkan12Features {
      ..Default::default()
    };
    let mut features13 = vk::PhysicalDeviceVulkan13Features {
      synchronization2: vk::TRUE,
      ..Default::default()
    };

    features12.p_next = &mut features13 as *mut vk::PhysicalDeviceVulkan13Features as *mut c_void;

    #[allow(deprecated)]
    let create_info = vk::DeviceCreateInfo {
      s_type: vk::StructureType::DEVICE_CREATE_INFO,
      p_queue_create_infos: queue_create_infos.as_ptr(),
      queue_create_info_count: unique_queue_size as u32,
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
      unsafe { instance.create_device(**physical_device, &create_info, None) }.map_err(
        |vkerr| match vkerr {
          vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
            DeviceCreationError::OutOfMemory(vkerr.into())
          }
          vk::Result::ERROR_INITIALIZATION_FAILED => {
            DeviceCreationError::VulkanInitializationFailed
          }
          vk::Result::ERROR_DEVICE_LOST => DeviceCreationError::DeviceLost,
          _ => panic!("Unhandled device creation error: {:?}", vkerr),
        },
      )?;

    let queues = unsafe {
      let queue_create_infos = &queue_create_infos[0..unique_queue_size];
      super::queues::retrieve_single_queues(
        &device,
        &physical_device.queue_families,
        queue_create_infos,
      )
    };
    debug_print_queues(physical_device, &queues).unwrap();

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

impl ManuallyDestroyed for Device {
  unsafe fn destroy_self(&self) {
    self.destroy_device(None);
  }
}

fn debug_print_queues(physical_device: &PhysicalDevice, queues: &SingleQueues) -> std::fmt::Result {
  let queue_family_properties = &physical_device.queue_family_properties;

  let mut output = String::from("\nAllocated queue properties:");

  let write_full = |output: &mut String, label, queue: Queue, family: vk::QueueFamilyProperties| {
    output.write_fmt(format_args!(
      "\n    <{}>:
        Address: {:?}
        Queue family index: {}
        Family's internal queue index: {}
        Queue family flags: {:?}
        image_transfer_granularity: ({}, {}, {})",
      label,
      queue.handle,
      queue.family_index,
      queue.index_in_family,
      family.queue_flags,
      family.min_image_transfer_granularity.width,
      family.min_image_transfer_granularity.height,
      family.min_image_transfer_granularity.depth
    ))
  };

  {
    write_full(
      &mut output,
      super::GRAPHICS_QUEUE_LABEL.to_str().unwrap(),
      queues.graphics,
      queue_family_properties[queues.graphics.family_index as usize],
    )?;

    {
      let transfer_equal_to_graphics = queues.graphics.handle == queues.transfer.handle;
      if transfer_equal_to_graphics {
        output.write_fmt(format_args!(
          "\n    <{}>: Same as <{}>",
          super::TRANSFER_QUEUE_LABEL.to_str().unwrap(),
          super::GRAPHICS_QUEUE_LABEL.to_str().unwrap()
        ))?;
      } else {
        write_full(
          &mut output,
          super::TRANSFER_QUEUE_LABEL.to_str().unwrap(),
          queues.transfer,
          queue_family_properties[queues.transfer.family_index as usize],
        )?;
      }
    }
  }

  log::debug!("{}", output);

  Ok(())
}
