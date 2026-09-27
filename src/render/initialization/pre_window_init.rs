use raw_window_handle::{HandleError, HasDisplayHandle};
use vkinitialization::{InstanceCreationError, InstanceOptionalExtensions};
use vkobjects::ManuallyDestroyed;
use winit::event_loop::{ActiveEventLoop, EventLoop};

use crate::{
  asset_loader::{texture_loader::TextureData, LoadedModels},
  render::{errors::InitializationError, SyncRenderer},
};
use std::mem;

use std::{
  mem::ManuallyDrop,
  {self},
};

pub struct RenderInit {
  pub entry: ManuallyDrop<ash::Entry>,
  pub instance: ManuallyDrop<ash::Instance>,
  #[cfg(feature = "vl")]
  pub debug_utils: ManuallyDrop<vkinitialization::DebugUtils>,

  pub loaded_models: ManuallyDrop<LoadedModels>,
  pub texture_data: ManuallyDrop<TextureData>,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderInitError {
  #[error("Failed to create a Vulkan Instance")]
  InstanceCreationFailed(#[source] InstanceCreationError),

  #[error("Failed to get display handle")]
  DisplayHandle(#[source] HandleError),

  #[error(transparent)]
  IOError(#[from] std::io::Error),
  #[error("Failed to load models\n{0}\nPath: {1}")]
  ModelLoadFailed(#[source] obj::ObjError, &'static str),

  #[error("Image error: {0}")]
  ImageError(#[from] image::ImageError),
}

impl From<InstanceCreationError> for RenderInitError {
  fn from(value: InstanceCreationError) -> Self {
    RenderInitError::InstanceCreationFailed(value)
  }
}

impl RenderInit {
  pub fn new(
    event_loop: &EventLoop<()>,
    models: LoadedModels,
    texture_data: TextureData,
  ) -> Result<Self, RenderInitError> {
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
      entry: ManuallyDrop::new(entry),
      instance: ManuallyDrop::new(instance),
      #[cfg(feature = "vl")]
      debug_utils: ManuallyDrop::new(debug_utils),

      loaded_models: ManuallyDrop::new(models),
      texture_data: ManuallyDrop::new(texture_data),
    })
  }

  pub fn start(
    mut self,
    event_loop: &ActiveEventLoop,
  ) -> Result<SyncRenderer, InitializationError> {
    let entry = unsafe { ManuallyDrop::take(&mut self.entry) };
    let instance = unsafe { ManuallyDrop::take(&mut self.instance) };
    #[cfg(feature = "vl")]
    let debug_utils = unsafe { ManuallyDrop::take(&mut self.debug_utils) };
    let models = unsafe { ManuallyDrop::take(&mut self.loaded_models) };
    let mut texture_data = unsafe { ManuallyDrop::take(&mut self.texture_data) };
    mem::forget(self);

    let renderer = SyncRenderer::new(
      entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      event_loop,
      &models,
      &mut texture_data,
    );

    renderer
  }
}

impl Drop for RenderInit {
  fn drop(&mut self) {
    unsafe {
      #[cfg(feature = "vl")]
      self.debug_utils.destroy_self();
      self.instance.destroy_self();

      ManuallyDrop::drop(&mut self.entry);
      ManuallyDrop::drop(&mut self.instance);
      #[cfg(feature = "vl")]
      ManuallyDrop::drop(&mut self.debug_utils);
      ManuallyDrop::drop(&mut self.loaded_models);
      ManuallyDrop::drop(&mut self.texture_data);
    }
  }
}
