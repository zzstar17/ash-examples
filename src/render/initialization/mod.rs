pub mod device;
mod entry;
mod instance;
mod pre_window_init;
mod surface;

#[cfg(feature = "vl")]
mod validation_layers;

use std::ffi::CStr;

pub use entry::get_entry;
pub use instance::{create_instance, InstanceCreationError};
pub use pre_window_init::{RenderInit, RenderInitError};
pub use surface::{Surface, SurfaceError};
#[cfg(feature = "vl")]
pub use validation_layers::{DebugUtils, DebugUtilsMarker};

static SURFACE_MAINTENANCE_EXT_NAME: &CStr = c"VK_KHR_surface_maintenance1";
static SWAPCHAIN_MAINTENANCE_EXT_NAME: &CStr = c"VK_KHR_swapchain_maintenance1";
