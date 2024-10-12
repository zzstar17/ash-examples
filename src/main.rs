mod entry;
mod errors;
mod instance;
mod utility;

// validation layers module will only exist if validation layers are enabled
#[cfg(feature = "vl")]
mod validation_layers;

use ash::vk;
use instance::InstanceCreationError;
use std::ffi::CStr;

// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
#[cfg(feature = "vl")]
const VALIDATION_LAYERS: [&CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];
#[cfg(feature = "vl")]
const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];

// Vulkan API version required to run the program
// You may have to use an older API version if you want to support devices that do not yet support
// the recent versions. You can see in the documentation what is the minimum supported version
// for each extension, feature or API call.
const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

// somewhat arbitrary
static APPLICATION_NAME: &CStr = c"Vulkan Instance Creation";
const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

fn run_app() -> Result<(), InstanceCreationError> {
  // initialize env_logger with debug if validation layers are enabled, warn otherwise
  #[cfg(feature = "vl")]
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
  #[cfg(not(feature = "vl"))]
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

  let entry: ash::Entry = unsafe { entry::get_entry() };

  #[cfg(feature = "vl")]
  let (instance, mut debug_utils) = instance::create_instance(&entry)?;
  #[cfg(not(feature = "vl"))]
  let instance = instance::create_instance(&entry)?;

  println!("Successfully created an instance!");

  log::debug!("Destroying objects");
  unsafe {
    #[cfg(feature = "vl")]
    debug_utils.destroy_self();
    instance.destroy_instance(None);
  }

  Ok(())
}

fn main() {
  if let Err(err) = run_app() {
    eprintln!("Instance creation failed:\n    {}", err);
  }
}
