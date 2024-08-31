use ash::vk;
use raw_window_handle::HandleError;

use super::{
  allocator::{AllocationError, DeviceMemoryInitializationError},
  initialization::InstanceCreationError,
  pipelines::{PipelineCacheError, PipelineCreationError},
  swapchain::{AcquireNextImageError, SwapchainCreationError},
};

pub fn error_chain_fmt(
  e: &impl std::error::Error,
  f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
  writeln!(f, "{}\nCauses:", e)?;
  let mut current = e.source();
  while let Some(cause) = current {
    writeln!(f, "  {}", cause)?;
    current = cause.source();
  }
  Ok(())
}

#[derive(thiserror::Error, Debug, Clone, Copy)]
pub enum OutOfMemoryError {
  #[error("Out of device memory")]
  OutOfDeviceMemory,
  #[error("Out of host memory")]
  OutOfHostMemory,
}

impl From<vk::Result> for OutOfMemoryError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => OutOfMemoryError::OutOfDeviceMemory,
      vk::Result::ERROR_OUT_OF_HOST_MEMORY => OutOfMemoryError::OutOfHostMemory,
      _ => {
        panic!("Invalid vk::Result to OutOfMemoryError cast: {:?}", value);
      }
    }
  }
}

impl From<OutOfMemoryError> for vk::Result {
  fn from(value: OutOfMemoryError) -> Self {
    match value {
      OutOfMemoryError::OutOfDeviceMemory => vk::Result::ERROR_OUT_OF_DEVICE_MEMORY,
      OutOfMemoryError::OutOfHostMemory => vk::Result::ERROR_OUT_OF_HOST_MEMORY,
    }
  }
}

#[derive(Debug, thiserror::Error)]
pub enum WindowError {
  #[error("OS error")]
  OsError(#[source] winit::error::OsError),
  #[error("Failed to get handle")]
  HandleError(#[source] HandleError),
}

#[derive(thiserror::Error, Debug, Clone, Copy)]
#[error("Vulkan returned ERROR_DEVICE_LOST. See https://docs.vulkan.org/spec/latest/chapters/devsandqueues.html#devsandqueues-lost-device")]
pub struct DeviceIsLost;

#[derive(thiserror::Error, Debug, Clone, Copy)]
pub enum QueueSubmitError {
  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),
  #[error(transparent)]
  DeviceIsLost(#[from] DeviceIsLost),
}

impl From<vk::Result> for QueueSubmitError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
        QueueSubmitError::OutOfMemory(value.into())
      }
      vk::Result::ERROR_DEVICE_LOST => QueueSubmitError::DeviceIsLost(DeviceIsLost {}),
      _ => {
        panic!("Invalid vk::Result to QueueSubmitError cast: {:?}", value);
      }
    }
  }
}

impl From<QueueSubmitError> for DeviceMemoryInitializationError {
  fn from(value: QueueSubmitError) -> Self {
    match value {
      QueueSubmitError::DeviceIsLost(_) => {
        DeviceMemoryInitializationError::DeviceIsLost(DeviceIsLost {})
      }
      QueueSubmitError::OutOfMemory(v) => v.into(),
    }
  }
}

#[derive(Debug, thiserror::Error)]
pub enum SwapchainRecreationError {
  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),
  #[error("Allocation error:\n{0}")]
  AllocationError(#[from] AllocationError),
  #[error("Failed to create a swapchain: {0}")]
  SwapchainError(#[from] SwapchainCreationError),
  #[error("Failed to create a pipeline: {0}")]
  PipelineCreationError(#[from] PipelineCreationError),
}

#[derive(thiserror::Error)]
pub enum InitializationError {
  #[error("Instance creation failed: {0}")]
  InstanceCreationFailed(#[from] InstanceCreationError),

  #[error("No physical device is compatible with the application requirements")]
  NoCompatibleDevices,

  #[error(transparent)]
  WindowError(#[from] WindowError),

  #[error("Image error: {0}")]
  ImageError(#[from] image::ImageError),

  #[error("Failed to allocate memory for some buffer or image\n{0}")]
  AllocationFailed(#[from] DeviceMemoryInitializationError),

  #[error("Ran out of memory while issuing some command or creating memory: {0}")]
  GenericOutOfMemoryError(#[from] OutOfMemoryError),

  #[error("Failed to create swapchain:\n{0}")]
  SwapchainCreationFailed(#[from] SwapchainCreationError),

  #[error("Failed to create pipelines:\n{0}")]
  PipelineCreationFailed(#[from] PipelineCreationError),

  #[error(transparent)]
  IOError(#[from] std::io::Error),

  // undefined behavior / driver or application bug (see vl)
  #[error(transparent)]
  DeviceIsLost(#[from] DeviceIsLost),
  #[error("Vulkan returned ERROR_UNKNOWN")]
  Unknown,
}
impl std::fmt::Debug for InitializationError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

impl From<winit::error::OsError> for InitializationError {
  fn from(value: winit::error::OsError) -> Self {
    InitializationError::WindowError(WindowError::OsError(value))
  }
}

impl From<AllocationError> for InitializationError {
  fn from(value: AllocationError) -> Self {
    InitializationError::AllocationFailed(value.into())
  }
}

impl From<HandleError> for InitializationError {
  fn from(value: HandleError) -> Self {
    InitializationError::WindowError(WindowError::HandleError(value))
  }
}

impl From<PipelineCacheError> for InitializationError {
  fn from(value: PipelineCacheError) -> Self {
    match value {
      PipelineCacheError::IOError(err) => InitializationError::IOError(err),
      PipelineCacheError::OutOfMemoryError(err) => InitializationError::from(err),
    }
  }
}

impl From<vk::Result> for InitializationError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
        OutOfMemoryError::from(value).into()
      }
      vk::Result::ERROR_DEVICE_LOST => InitializationError::DeviceIsLost(DeviceIsLost {}),
      vk::Result::ERROR_UNKNOWN => InitializationError::Unknown,
      // validation layers may say more on this
      vk::Result::ERROR_INITIALIZATION_FAILED => InitializationError::Unknown,
      _ => {
        log::error!(
          "Unhandled vk::Result {} during general initialization",
          value
        );
        InitializationError::Unknown
      }
    }
  }
}

#[derive(thiserror::Error)]
pub enum FrameRenderError {
  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),

  #[error("Device is lost")]
  DeviceLost,

  #[error("Failed to acquire swapchain image: {0}")]
  FailedToAcquireSwapchainImage(#[from] AcquireNextImageError),

  #[error("Failed to recreate swapchain: {0}")]
  FailedToRecreateSwapchain(#[from] SwapchainRecreationError),
}
impl std::fmt::Debug for FrameRenderError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

impl From<vk::Result> for FrameRenderError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        FrameRenderError::OutOfMemory(OutOfMemoryError::from(value))
      }
      vk::Result::ERROR_DEVICE_LOST => FrameRenderError::DeviceLost,
      _ => panic!("Invalid cast from vk::Result to FrameRenderError"),
    }
  }
}

impl From<QueueSubmitError> for InitializationError {
  fn from(value: QueueSubmitError) -> Self {
    match value {
      QueueSubmitError::DeviceIsLost(_) => InitializationError::DeviceIsLost(DeviceIsLost {}),
      QueueSubmitError::OutOfMemory(v) => v.into(),
    }
  }
}

#[derive(thiserror::Error)]
pub enum ImageError {
  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),

  #[error("Image Error")]
  ImageError(#[source] image::ImageError),
}
impl std::fmt::Debug for ImageError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

impl From<image::ImageError> for ImageError {
  fn from(value: image::ImageError) -> Self {
    Self::ImageError(value)
  }
}
impl From<OutOfMemoryError> for ImageError {
  fn from(value: OutOfMemoryError) -> Self {
    Self::OutOfMemory(value)
  }
}
