use ash::vk::{self};

use std::{ffi::CStr, marker::PhantomData, os::raw::c_void, ptr};

use crate::render::{
  device_destroyable::ManuallyDestroyed, errors::OutOfMemoryError,
  initialization::device::SingleQueues, VALIDATION_LAYERS,
};

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
    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL => "[General] ",
    vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE => "[Performance]\n",
    vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION => "[Validation]\n",
    _ => "[Unknown] ",
  };
  let message = CStr::from_ptr((*p_callback_data).p_message);
  let message = format!("{}{}", types, message.to_str().unwrap());
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

  pub unsafe fn destroy_self(&self) {
    self
      .loader
      .destroy_debug_utils_messenger(self.messenger, None);
  }
}

impl ManuallyDestroyed for DebugUtils {
  unsafe fn destroy_self(&self) {
    self.destroy_self();
  }
}

pub struct DebugUtilsMarker {
  loader: ash::ext::debug_utils::Device,
}

impl DebugUtilsMarker {
  pub fn new(instance: &ash::Instance, device: &ash::Device) -> Self {
    Self {
      loader: ash::ext::debug_utils::Device::new(instance, device),
    }
  }

  pub unsafe fn set_queue_labels(&self, queues: SingleQueues) {
    {
      let label_info = vk::DebugUtilsLabelEXT::default()
        .label_name(crate::render::initialization::device::GRAPHICS_QUEUE_LABEL)
        .color([1.0, 0.0, 0.0, 1.0]);
      self
        .loader
        .queue_insert_debug_utils_label(*queues.graphics, &label_info);
    }
    {
      let label_info = vk::DebugUtilsLabelEXT::default()
        .label_name(crate::render::initialization::device::TRANSFER_QUEUE_LABEL)
        .color([0.0, 0.0, 1.0, 1.0]);
      self
        .loader
        .queue_insert_debug_utils_label(*queues.transfer, &label_info);
    }
  }

  pub unsafe fn set_obj_name(
    &self,
    object_type: vk::ObjectType,
    object_handle: u64,
    name: &CStr,
  ) -> Result<(), OutOfMemoryError> {
    let info = vk::DebugUtilsObjectNameInfoEXT {
      s_type: vk::StructureType::DEBUG_UTILS_OBJECT_NAME_INFO_EXT,
      p_next: ptr::null(),
      object_type,
      object_handle,
      p_object_name: name.as_ptr(),
      _marker: PhantomData,
    };
    self
      .loader
      .set_debug_utils_object_name(&info)
      .map_err(|err| err.into())
  }
}
