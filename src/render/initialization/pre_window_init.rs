use raw_window_handle::{HandleError, HasDisplayHandle};
use vkinitialization::{InstanceCreationError, InstanceOptionalExtensions};
use vkobjects::ManuallyDestroyed;
use winit::event_loop::{ActiveEventLoop, EventLoop};

use crate::render::{errors::InitializationError, renderer::Renderer, SyncRenderer};
use std::mem;

use std::{
  self,
  mem::MaybeUninit,
  ptr::{self, addr_of_mut},
};

pub struct RenderInit {
  pub entry: ash::Entry,
  pub instance: ash::Instance,
  #[cfg(feature = "vl")]
  pub debug_utils: vkinitialization::DebugUtils,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderInitError {
  #[error("Failed to create a Vulkan Instance")]
  InstanceCreationFailed(#[source] InstanceCreationError),

  #[error("Failed to get display handle")]
  DisplayHandle(#[source] HandleError),
}

impl From<InstanceCreationError> for RenderInitError {
  fn from(value: InstanceCreationError) -> Self {
    RenderInitError::InstanceCreationFailed(value)
  }
}

impl RenderInit {
  pub fn new(event_loop: &EventLoop<()>) -> Result<Self, RenderInitError> {
    let entry: ash::Entry = unsafe { vkinitialization::get_entry() };

    let display_handle = event_loop
      .display_handle()
      .map_err(RenderInitError::DisplayHandle)?;

    let app_info = crate::render::initialization::get_app_info();
    let optional_extensions = InstanceOptionalExtensions {
      get_surface_capabilities2: true,
      surface_maintenance1: true,
    };
    #[cfg(feature = "vl")]
    let (instance, _instance_optional_extensions, debug_utils) =
      vkinitialization::create_instance(&entry, app_info, optional_extensions, display_handle)?;
    #[cfg(not(feature = "vl"))]
    let (instance, _instance_optional_extensions) =
      vkinitialization::create_instance(&entry, app_info, optional_extensions, display_handle)?;

    Ok(Self {
      entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
    })
  }

  pub fn start(self, event_loop: &ActiveEventLoop) -> Result<SyncRenderer, InitializationError> {
    let renderer = Renderer::initialize(self, event_loop)?;
    SyncRenderer::new(renderer)
  }

  // take values out without calling drop
  #[cfg(feature = "vl")]
  pub fn deconstruct(mut self) -> (ash::Entry, ash::Instance, vkinitialization::DebugUtils) {
    unsafe {
      // could't find a less stupid way of doing this
      let mut entry: MaybeUninit<ash::Entry> = MaybeUninit::uninit();
      ptr::copy_nonoverlapping(addr_of_mut!(self.entry), entry.as_mut_ptr(), 1);
      let mut instance = MaybeUninit::uninit();
      ptr::copy_nonoverlapping(addr_of_mut!(self.instance), instance.as_mut_ptr(), 1);
      let mut debug_utils = MaybeUninit::uninit();
      ptr::copy_nonoverlapping(addr_of_mut!(self.debug_utils), debug_utils.as_mut_ptr(), 1);

      mem::forget(self);
      (
        entry.assume_init(),
        instance.assume_init(),
        debug_utils.assume_init(),
      )
    }
  }

  #[cfg(not(feature = "vl"))]
  pub fn deconstruct(mut self) -> (ash::Entry, ash::Instance) {
    unsafe {
      let mut entry: MaybeUninit<ash::Entry> = MaybeUninit::uninit();
      ptr::copy_nonoverlapping(addr_of_mut!(self.entry), entry.as_mut_ptr(), 1);
      let mut instance = MaybeUninit::uninit();
      ptr::copy_nonoverlapping(addr_of_mut!(self.instance), instance.as_mut_ptr(), 1);

      mem::forget(self);
      (entry.assume_init(), instance.assume_init())
    }
  }
}

impl Drop for RenderInit {
  fn drop(&mut self) {
    unsafe {
      #[cfg(feature = "vl")]
      self.debug_utils.destroy_self();
      self.instance.destroy_self();
    }
  }
}
