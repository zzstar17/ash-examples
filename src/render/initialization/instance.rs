use ash::vk;
use itertools::Itertools;
use raw_window_handle::DisplayHandle;
use std::{
  ffi::{c_char, c_void, CStr},
  marker::PhantomData,
  ptr,
};

use crate::{
  render::{errors::OutOfMemoryError, TARGET_API_VERSION},
  utility, APPLICATION_NAME, APPLICATION_VERSION,
};

#[derive(thiserror::Error, Debug)]
pub enum InstanceCreationError {
  #[error("Vulkan implementation API maximum supported version ({0}) is less than the one targeted by the application ({1})")]
  UnsupportedApiVersion(String, String),

  #[error("Missing instance extension \"{0}\"")]
  MissingExtension(String),
  // validation layers will be skipped if not available
  #[error("Missing instance layer \"{0}\"")]
  MissingLayer(String),

  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),

  #[error(
    "Vulkan returned VK_ERROR_INITIALIZATION_FAILED.\
The instance could not be created for implementation-specific reasons."
  )]
  VulkanInitializationFailed,
}

// checks if entry supports the target API version
fn check_api_version(entry: &ash::Entry) -> Result<(), InstanceCreationError> {
  let max_supported_version = match unsafe { entry.try_enumerate_instance_version() } {
    // Vulkan 1.1+
    Ok(opt) => match opt {
      Some(version) => version,
      None => vk::API_VERSION_1_0,
    },
    // Vulkan 1.0
    Err(_) => vk::API_VERSION_1_0,
  };

  log::info!(
    "Vulkan library max supported version: {}",
    utility::parse_vulkan_api_version(max_supported_version)
  );

  if max_supported_version < TARGET_API_VERSION {
    return Err(InstanceCreationError::UnsupportedApiVersion(
      utility::parse_vulkan_api_version(max_supported_version),
      utility::parse_vulkan_api_version(TARGET_API_VERSION),
    ));
  }

  Ok(())
}

fn get_app_info<'a>() -> vk::ApplicationInfo<'a> {
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

#[cfg(feature = "vl")]
pub fn create_instance(
  entry: &ash::Entry,
  display_handle: DisplayHandle,
) -> Result<(ash::Instance, super::validation_layers::DebugUtils), InstanceCreationError> {
  use super::validation_layers::{self, DebugUtils};
  use crate::render::ADDITIONAL_VALIDATION_FEATURES;

  let app_info = get_app_info();

  let surface_extensions = ash_window::enumerate_required_extensions(display_handle.as_raw())
    .map_err(OutOfMemoryError::from)?;
  let mut extensions = Vec::with_capacity(surface_extensions.len() + 1);
  extensions.extend(surface_extensions.iter());
  extensions.push(ash::ext::debug_utils::NAME.as_ptr());

  let layers_str = validation_layers::get_supported_validation_layers(entry)
    .map_err(|err| InstanceCreationError::OutOfMemory(err.into()))?;
  let layers: Vec<*const c_char> = layers_str.iter().map(|name| name.as_ptr()).collect();

  let debug_create_info = DebugUtils::get_debug_messenger_create_info();

  // enable/disable some validation features by passing a ValidationFeaturesEXT struct
  let additional_features = vk::ValidationFeaturesEXT {
    s_type: vk::StructureType::VALIDATION_FEATURES_EXT,
    p_next: &debug_create_info as *const vk::DebugUtilsMessengerCreateInfoEXT as *const c_void,
    enabled_validation_feature_count: ADDITIONAL_VALIDATION_FEATURES.len() as u32,
    p_enabled_validation_features: ADDITIONAL_VALIDATION_FEATURES.as_ptr(),
    disabled_validation_feature_count: 0,
    p_disabled_validation_features: ptr::null(),
    _marker: PhantomData,
  };

  let instance = create_instance_checked(
    entry,
    app_info,
    &extensions,
    &layers,
    &additional_features as *const vk::ValidationFeaturesEXT as *const c_void,
  )?;

  log::debug!("Creating Debug Utils");
  let debug_utils = DebugUtils::create(entry, &instance, debug_create_info)?;

  Ok((instance, debug_utils))
}

#[cfg(not(feature = "vl"))]
pub fn create_instance(
  entry: &ash::Entry,
  display_handle: DisplayHandle,
) -> Result<ash::Instance, InstanceCreationError> {
  check_api_version(entry)?;

  let app_info = get_app_info();
  let extensions = ash_window::enumerate_required_extensions(display_handle.as_raw())
    .map_err(|vkerr| OutOfMemoryError::from(vkerr))?;
  let layers = [];
  create_instance_checked(entry, app_info, &extensions, &layers, ptr::null())
}

// check if extensions are layers are present and then create a vk instance
// (safety: extensions and layers should be valid cstrings)
fn create_instance_checked(
  entry: &ash::Entry,
  app_info: vk::ApplicationInfo,
  extensions: &[*const c_char],
  layers: &[*const c_char],
  p_next: *const c_void,
) -> Result<ash::Instance, InstanceCreationError> {
  check_api_version(entry)?;

  log::debug!(
    "[Instance creation] Requested Instance Extensions: {}",
    extensions
      .iter()
      .map(|ptr| format!("{:?}", unsafe { CStr::from_ptr(*ptr) }))
      .join(", "),
  );
  // check that all extensions are available
  {
    let available = unsafe { entry.enumerate_instance_extension_properties(None) }
      .map_err(|err| InstanceCreationError::OutOfMemory(err.into()))?;
    for &ptr in extensions {
      let extension = unsafe { CStr::from_ptr(ptr) };
      if !available
        .iter()
        .filter_map(|av| av.extension_name_as_c_str().ok())
        .any(|av| av == extension)
      {
        return Err(InstanceCreationError::MissingExtension(String::from(
          extension.to_str().unwrap(),
        )));
      }
    }
  };

  log::debug!(
    "[Instance creation] Requested Instance Layers: {}",
    layers
      .iter()
      .map(|ptr| format!("{:?}", unsafe { CStr::from_ptr(*ptr) }))
      .join(", "),
  );
  // check that all layers are available
  {
    let available = unsafe { entry.enumerate_instance_layer_properties() }
      .map_err(|err| InstanceCreationError::OutOfMemory(err.into()))?;
    for &ptr in layers {
      let layer = unsafe { CStr::from_ptr(ptr) };
      if !available
        .iter()
        .filter_map(|av| av.layer_name_as_c_str().ok())
        .any(|av| av == layer)
      {
        return Err(InstanceCreationError::MissingLayer(String::from(
          layer.to_str().unwrap(),
        )));
      }
    }
  };

  let mut create_info = vk::InstanceCreateInfo::default()
    .application_info(&app_info)
    .enabled_extension_names(extensions)
    .enabled_layer_names(layers);
  create_info.p_next = p_next;

  log::debug!("[Instance creation] Creating Instance");
  let instance: ash::Instance =
    unsafe { entry.create_instance(&create_info, None) }.map_err(|err| match err {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        InstanceCreationError::OutOfMemory(err.into())
      }
      vk::Result::ERROR_INITIALIZATION_FAILED => InstanceCreationError::VulkanInitializationFailed,
      // other results have been checked
      _ => panic!(),
    })?;

  log::debug!("[Instance creation] Successfully created the Vulkan Instance");
  Ok(instance)
}
