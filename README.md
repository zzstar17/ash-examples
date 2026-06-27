# Instance creation

This example covers creating an entry to the Vulkan library and initializing an Vulkan instance with a set of extensions and layers. This also includes the functionality of enabling/disabling validation layers with the help of a Debug Messenger.

This example uses ash version `0.38`.

You can run this example with:

`cargo run`

## Helpful resources

If you are new to Vulkan, consider reading the initial part of the [Vulkan Guide](https://docs.vulkan.org/guide/latest/what_is_vulkan.html) and the [Vulkan Tutorial](https://docs.vulkan.org/tutorial/latest/03_Drawing_a_triangle/00_Setup/01_Instance.html) up until the instance chapter. If you don't mind reading the specification, then also read [Fundamentals](https://docs.vulkan.org/spec/latest/chapters/fundamentals.html), [Initialization](https://docs.vulkan.org/spec/latest/chapters/initialization.html) and possibly also [Debugging](https://docs.vulkan.org/spec/latest/chapters/debugging.html#debugging-debug-messengers).

### Ash Entry

Ash contains an [Entry struct](https://docs.rs/ash/latest/ash/struct.Entry.html) that aids in linking / loading the main Vulkan library and serves as the starting point for initializing all other Vulkan objects. Linking is infallible but makes it so that the resulting binary cannot start in environments that do not support Vulkan.

### API Versions, extensions and layers

Most of Vulkan's functionality is included in the Core 1.0 version. This functionality can extended with the use of newer API versions and extensions, however it is not guaranteed that these will always be supported by the machine's driver or the Vulkan device (like the GPU). Some extensions are also sometimes "promoted" into newer API versions, so targeting that version will include that extension by default.

While reading the reference for some Vulkan command, make sure it is provided by the API version your application targets or, if the command was introduced by some extension, that extension is actually enabled.

Vulkan is a layered API, with the core functionality being at the lowest layer. Additional layers can for example intercept core function calls and add their own functionality (like profiling, debugging, tracing or validation), which makes them somewhat more powerful than extensions. These layers must be installed separately (although some also come with the Vulkan SDK) and must also be explicitly enabled.

Vulkan by default comes with almost no validation, and so validation layers should always be enabled during development to make sure the application performs valid usage with no undefined behavior or other errors, although this is not infallible. The main validation layer is `VK_LAYER_KHRONOS_validation`, which is also used in this example.

LunarG has tool called the [Vulkan Configurator](https://vulkan.lunarg.com/doc/view/1.4.350.0/windows/vkconfig.html) (vkconfig) which helps configure and enable layers using different presets, instead of having to do it programmatically.

### Instance creation

An instance stores Vulkan application state and is used in most API calls that are not related to any specific device.

Creating an instance takes the following parameters:

- An API version: The desired target API version. Subsequent Vulkan commands may only be used if they are supported by this version.
- A `vk::ApplicationInfo`: This contains the desired target API version, as well as some other not that important information about the application like name and engine version. Vulkan commands may only be used if they are supported by the provided API version version.
- A list of instance extensions: These extensions are related to the instance, meaning they mostly depend on the implementation of the Vulkan driver and not the actual Vulkan devices like installed graphics cards. These extensions must actually be supported by the execution system to be able to be enabled.
- A list of layers: Layers can be enabled as long as they are installed. These are usually only enabled during debug builds, and we provide an example by only enabling the main validation layer when the `vl` cargo feature is enabled.

### Enabling validation layers

As said before, Vulkan by design doesn't check for undefined behavior like missuses of the API or out of bounds memory accesses, or other performance and memory issues. In order to mitigate that, validation layers can be enabled in some builds to detect and log errors and other smaller issues. These layers are not part of the Vulkan API and may have to be installed separately, but in return allows you to customize when and what validation is enabled.

Validation can be performance heavy, so generally only a subset of its functionality is enabled at a time. For example, some synchronization bugs can only occur in environments that are as fast as release builds because of timing issues, where in that case only the part of validation that is required to catch those specific bugs will be enabled. In release builds the validation layers can be completely disabled.

The `VK_LAYER_KHRONOS_validation` layer is Vulkan's main validation layer and it can validate input and detect malpractices and other missuses of the API. It also contains some features related to debugging shader code and detecting synchronization issues. See https://docs.vulkan.org/guide/latest/development_tools.html#_vulkan_layers for more information and other info about other related validation layers.

The actual layer does not actually print the errors to stdout, so we enable a [special extension](https://docs.vulkan.org/samples/latest/samples/extensions/debug_utils/README.html) that enables the use of a `vk::DebugUtilsMessengerEXT` object. This object enables us to receive, parse and forward messages from validation to this example's logging library. In order for messages to also be received during instance creation, this object's creation info is also passed to the instance creation info in the `p_next` chain.

## Some code explanations

As the application is small, you may want to first take a look at `./src/main.rs` and try to follow the code as it is from here.

The file `./src/instance.rs` has all the code responsible for checking and creating the instance. Its main function is:

```rust
// (safety: extensions and layers should be valid cstrings)
fn create_instance_checked(
  entry: &ash::Entry,
  app_info: vk::ApplicationInfo,
  extensions: &[*const c_char],
  layers: &[*const c_char],
  p_next: *const c_void,
) -> Result<ash::Instance, InstanceCreationError>
```

Which checks if all layers and extensions are valid and the desired API version is supported, or returns an error otherwise. It also takes an optional `p_next` pointer that may add some extended functionality not tested by this function.

The `create_instance_checked` function is called by another function that is public and is used in main. In case that no validation layers are to be enabled, this other function simply passes default or empty parameters:

```rust
#[cfg(not(feature = "vl"))]
pub fn create_instance(entry: &ash::Entry) -> Result<ash::Instance, InstanceCreationError> {
  check_api_version(entry)?;

  let app_info = get_app_info();
  let extensions = [];
  let layers = [];
  create_instance_checked(entry, app_info, &extensions, &layers, ptr::null())
}
```

However, when validation layers are enabled, it takes some constants defined in main:

```rust
// validation layers names should be valid cstrings (not contain null bytes nor invalid characters)
#[cfg(feature = "vl")]
const VALIDATION_LAYERS: [&CStr; 1] = [c"VK_LAYER_KHRONOS_validation"];
#[cfg(feature = "vl")]
const ADDITIONAL_VALIDATION_FEATURES: [vk::ValidationFeatureEnableEXT; 2] = [
  vk::ValidationFeatureEnableEXT::BEST_PRACTICES,
  vk::ValidationFeatureEnableEXT::SYNCHRONIZATION_VALIDATION,
];
```

Checks if the layers exists, and enables the extra features by passing a `vk::ValidationFeaturesEXT` struct in the `p_next` field.

```rust
#[cfg(feature = "vl")]
pub fn create_instance(
  entry: &ash::Entry,
) -> Result<(ash::Instance, crate::validation_layers::DebugUtils), InstanceCreationError> {
  use crate::{
    validation_layers::{self, DebugUtils},
    ADDITIONAL_VALIDATION_FEATURES,
  };

  let app_info = get_app_info();

  let extensions = vec![ash::ext::debug_utils::NAME.as_ptr()];

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
```

The previous function also creates a object called `DebugUtils`, which has the job of retrieving the messages written by the validation layers and forward them to the application normal logging implementation. Creating this debug messenger and making it work during instance creation is a bit convoluted as it requires passing the `vk::DebugUtilsMessengerCreateInfoEXT` struct as well as a special external function that does the actual message translation and forwarding.

```rust
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
    _ => "[Unknown]\n",
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
```

This type of debugging can be very extensive depending on your needs. Check for example https://docs.vulkan.org/spec/latest/chapters/debugging.html#debugging-debug-messengers.

## Cargo features

This example implements the following cargo features:

- `vl`: Enable validation layers.
- `load`: Load the system Vulkan Library at runtime.
- `link`: Link the system Vulkan Library at compile time.

`vl` and `load` are enabled by default. To disable them, pass `--no-default-features` to cargo.
For example:

`cargo run --release --no-default-features --features link`
