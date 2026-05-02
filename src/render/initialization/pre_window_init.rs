use raw_window_handle::{HandleError, HasDisplayHandle};
use winit::event_loop::{EventLoop, EventLoopWindowTarget};

use crate::render::{
  device_destroyable::ManuallyDestroyed, errors::InitializationError, renderer::Renderer,
  SyncRenderer,
};
use std::mem;

use super::InstanceCreationError;

use std::{
  self,
  mem::MaybeUninit,
  ptr::{self, addr_of_mut},
};

pub struct RenderInit {
  pub entry: ash::Entry,
  pub instance: ash::Instance,
  #[cfg(feature = "vl")]
  pub debug_utils: super::DebugUtils,
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
    let entry: ash::Entry = unsafe { super::get_entry() };

    let display_handle = event_loop
      .display_handle()
      .map_err(RenderInitError::DisplayHandle)?;

    #[cfg(feature = "vl")]
    let (instance, _instance_optional_extensions, debug_utils) =
      super::create_instance(&entry, display_handle)?;
    #[cfg(not(feature = "vl"))]
    let (instance, _instance_optional_extensions) = super::create_instance(&entry, display_handle)?;

    Ok(Self {
      entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
    })
  }

  pub fn start(
    self,
    event_loop: &EventLoopWindowTarget<()>,
  ) -> Result<SyncRenderer, InitializationError> {
    let renderer = Renderer::initialize(self, event_loop)?;
    SyncRenderer::new(renderer)
  }

  // take values out without calling drop
  #[cfg(feature = "vl")]
  pub fn deconstruct(mut self) -> (ash::Entry, ash::Instance, super::DebugUtils) {
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
