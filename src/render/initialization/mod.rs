mod device_selector;
mod post_window_init;
mod pre_window_init;

use ash::vk::{self};
pub use device_selector::select_physical_device;
use vkinitialization::device::{Queue, SingleQueues};

use std::{
  marker::PhantomData,
  ptr,
  sync::{Arc, Mutex},
};

pub use post_window_init::PostWindowInit;
pub use pre_window_init::{PreWindowInit, PreWindowInitError};

use crate::{
  render::{gpu_data::TEXTURE_FORMAT_FEATURES, TARGET_API_VERSION},
  APPLICATION_NAME, APPLICATION_VERSION,
};

pub struct SyncQueues {
  pub graphics: Arc<Mutex<Queue>>,
  pub compute: Arc<Mutex<Queue>>,
  pub transfer: Arc<Mutex<Queue>>,
}

pub struct GraphicsSyncQueues {
  pub graphics: Arc<Mutex<Queue>>,
  pub transfer: Arc<Mutex<Queue>>,
}

pub struct ComputeSyncQueues {
  pub compute: Arc<Mutex<Queue>>,
  pub transfer: Arc<Mutex<Queue>>,
}

impl SyncQueues {
  pub fn from_single_queues(queues: SingleQueues) -> Self {
    let (mutexes, mutex_assignment) = vkinitialization::device::create_mutexes_for_queues(&[
      queues.graphics,
      queues.compute,
      queues.transfer,
    ]);
    Self {
      graphics: mutexes[mutex_assignment[0]].clone(),
      compute: mutexes[mutex_assignment[1]].clone(),
      transfer: mutexes[mutex_assignment[2]].clone(),
    }
  }
}

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

pub fn format_is_supported(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
  format: vk::Format,
) -> bool {
  let properties =
    unsafe { instance.get_physical_device_format_properties(physical_device, format) };

  properties
    .optimal_tiling_features
    .contains(TEXTURE_FORMAT_FEATURES)
}
