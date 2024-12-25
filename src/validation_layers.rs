use ash::vk::{self};
use ash_destructor::{DeviceDestroyable, SelfDestroyable};

use std::{ffi::CStr, os::raw::c_void, ptr};

use crate::{errors::OutOfMemoryError, VALIDATION_LAYERS};

// returns a list of supported and unsupported instance layers
fn filter_supported(
  available: Vec<vk::LayerProperties>,
) -> (Vec<&'static CStr>, Vec<&'static CStr>) {
  VALIDATION_LAYERS.into_iter().partition(|&req| {
    available
      .iter()
      .filter_map(|av| av.layer_name_as_c_str().ok())
      .any(|av| av == req)
  })
}

// returns a subset of VALIDATION_LAYERS that are available
pub fn get_supported_validation_layers(
  entry: &ash::Entry,
) -> Result<Box<[&'static CStr]>, vk::Result> {
  log::info!("Querying Vulkan instance layers");
  let (available, unavailable) =
    filter_supported(unsafe { entry.enumerate_instance_layer_properties() }?);

  if !unavailable.is_empty() {
    log::error!(
      "Some requested validation layers are not available: {:?}",
      unavailable
    );
  }

  Ok(available.into_boxed_slice())
}

// can be extensively customized
unsafe extern "system" fn vulkan_debug_utils_callback(
  message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
  message_type: vk::DebugUtilsMessageTypeFlagsEXT,
  p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
  _p_user_data: *mut c_void,
) -> vk::Bool32 {
  let types = match message_type {
    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General]",
    vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]",
    vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]",
    _ => "[Unknown]",
  };
  let message = CStr::from_ptr((*p_callback_data).p_message);
  let message = format!("{} {}", types, message.to_str().unwrap());
  match message_severity {
    vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE => log::debug!("{message}"),
    vk::DebugUtilsMessageSeverityFlagsEXT::WARNING => log::warn!("{message}"),
    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR => log::error!("{message}"),
    vk::DebugUtilsMessageSeverityFlagsEXT::INFO => log::info!("{message}"),
    _ => log::warn!("<Unknown>: {message}"),
  }

  vk::FALSE
}

pub struct DebugUtils {
  loader: ash::ext::debug_utils::Instance,
  messenger: vk::DebugUtilsMessengerEXT,
}

impl DebugUtils {
  pub fn create(
    entry: &ash::Entry,
    instance: &ash::Instance,
    create_info: vk::DebugUtilsMessengerCreateInfoEXT,
  ) -> Result<Self, OutOfMemoryError> {
    let loader = ash::ext::debug_utils::Instance::new(entry, instance);

    let messenger = unsafe { loader.create_debug_utils_messenger(&create_info, None)? };

    Ok(Self { loader, messenger })
  }

  pub fn get_debug_messenger_create_info<'a>() -> vk::DebugUtilsMessengerCreateInfoEXT<'a> {
    vk::DebugUtilsMessengerCreateInfoEXT {
      flags: vk::DebugUtilsMessengerCreateFlagsEXT::empty(),
      message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
        | vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO
        | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
      message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
      pfn_user_callback: Some(vulkan_debug_utils_callback),
      p_user_data: ptr::null_mut(),
      ..Default::default()
    }
  }
}

impl DeviceDestroyable for DebugUtils {
  unsafe fn destroy_self_alloc(
    &self,
    _device: &ash::Device,
    allocation_callbacks: Option<&vk::AllocationCallbacks>,
  ) {
    self
      .loader
      .destroy_debug_utils_messenger(self.messenger, allocation_callbacks);
  }
}

impl SelfDestroyable for DebugUtils {
  unsafe fn destroy_self_alloc(&self, allocation_callbacks: Option<&vk::AllocationCallbacks>) {
    self
      .loader
      .destroy_debug_utils_messenger(self.messenger, allocation_callbacks);
  }
}
