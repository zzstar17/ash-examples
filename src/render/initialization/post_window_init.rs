use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use vkinitialization::{
  device::{Device, DeviceExtensions, DeviceFeatures, PhysicalDevice, SingleQueues},
  Surface,
};
use vkobjects::{destroy, utility::OnErr, ManuallyDestroyed};
use winit::{dpi::PhysicalSize, event_loop::ActiveEventLoop, window::Window};

use crate::{
  compute::ferris::Ferris,
  render::{initialization, InitializationError},
  INITIAL_WINDOW_HEIGHT, INITIAL_WINDOW_WIDTH, WINDOW_TITLE,
};

pub struct PostWindowInit {
  _entry: ash::Entry,
  pub instance: ash::Instance,
  #[cfg(feature = "vl")]
  pub debug_utils: vkinitialization::DebugUtils,
  #[cfg(feature = "vl")]
  pub debug_utils_marker: vkinitialization::DebugUtilsMarker,
  pub physical_device: PhysicalDevice,
  pub device: Device,
  pub queues: SingleQueues,

  pub window: Window,
  pub surface: Surface,
}

impl PostWindowInit {
  pub fn initialize(
    pre_window: super::PreWindowInit,
    event_loop: &ActiveEventLoop,
  ) -> Result<Self, InitializationError> {
    // having an error during window creation triggers pre_window drop
    let window_attributes = Window::default_attributes()
      .with_title(WINDOW_TITLE)
      .with_inner_size(PhysicalSize {
        width: INITIAL_WINDOW_WIDTH,
        height: INITIAL_WINDOW_HEIGHT,
      })
      .with_min_inner_size(PhysicalSize {
        width: Ferris::WIDTH,
        height: Ferris::HEIGHT,
      });
    // .with_resizable(false)
    let window = event_loop.create_window(window_attributes)?;

    #[cfg(feature = "vl")]
    let (entry, instance, debug_utils) = pre_window.deconstruct();
    #[cfg(not(feature = "vl"))]
    let (entry, instance) = pre_window.deconstruct();

    let destroy_instance = || unsafe {
      #[cfg(feature = "vl")]
      destroy!(&debug_utils);
      destroy!(&instance);
    };

    let surface = Surface::new(
      &entry,
      &instance,
      event_loop.display_handle()?,
      window.window_handle()?,
    )
    .on_err(|_| destroy_instance())?;

    // can return an error and can also return no devices
    let physical_device_creation = match unsafe {
      PhysicalDevice::select(&instance, &surface, initialization::select_physical_device)
    }
    .on_err(|_| destroy_instance())?
    {
      Some(tu) => tu,
      None => {
        destroy_instance();
        return Err(InitializationError::NoCompatibleDevices);
      }
    };

    let (device, queues) = Device::create(
      &instance,
      &physical_device_creation,
      DeviceExtensions {
        swapchain: true,
        ..Default::default()
      },
      DeviceExtensions {
        memory_priority: true,
        pageable_device_local_memory: true,
        swapchain_maintenance1: true,
        ..Default::default()
      },
      DeviceFeatures {
        synchronization2: true,
        ..Default::default()
      },
      DeviceFeatures {
        swapchain_maintenance1: true,
        ..Default::default()
      },
    )
    .on_err(|_| destroy_instance())?;

    let physical_device = physical_device_creation.physical_device;

    #[cfg(feature = "vl")]
    let debug_utils_marker = vkinitialization::DebugUtilsMarker::new(&instance, &device);
    #[cfg(feature = "vl")]
    unsafe {
      debug_utils_marker.set_queue_labels(queues);
    }

    Ok(Self {
      window,
      surface,
      _entry: entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      #[cfg(feature = "vl")]
      debug_utils_marker,
      physical_device,
      device,
      queues,
    })
  }
}

impl ManuallyDestroyed for PostWindowInit {
  unsafe fn destroy_self(&self) {
    unsafe {
      ManuallyDestroyed::destroy_self(&self.surface);
      ManuallyDestroyed::destroy_self(&self.device);

      #[cfg(feature = "vl")]
      {
        ManuallyDestroyed::destroy_self(&self.debug_utils);
      }
      ManuallyDestroyed::destroy_self(&self.instance);
    }
  }
}
