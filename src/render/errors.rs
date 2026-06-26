use ash::vk;
use raw_window_handle::HandleError;
use vkallocator::{
  AllocationError, DeviceMemoryInitializationError, HostAllocationError, HostMemorySyncError,
};
use vkinitialization::{
  device::{device_selector::PhysicalDeviceSelectionError, DeviceCreationError},
  InstanceCreationError,
};
use vkobjects::errors::{DeviceIsLost, OutOfMemoryError, QueueSubmitError};

use super::{
  graphics::swapchain::{AcquireNextImageError, SwapchainCreationError},
  pipelines::{PipelineCacheError, PipelineCreationError},
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

#[derive(Debug, thiserror::Error)]
pub enum WindowError {
  #[error("OS error")]
  OsError(#[source] winit::error::OsError),
  #[error("Failed to get handle")]
  HandleError(#[source] HandleError),
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
  #[error("Instance creation failed:\n    {0}")]
  InstanceCreationFailed(#[from] InstanceCreationError),

  #[error("An error occurred during device selection: {0}")]
  PhysicalDeviceSelectionError(#[from] PhysicalDeviceSelectionError),
  #[error("No physical device supports the application")]
  NoCompatibleDevices,
  #[error("An error occurred during the creation of the logical device:\n    {0}")]
  DeviceCreationError(#[from] DeviceCreationError),

  #[error(transparent)]
  WindowError(#[from] WindowError),

  #[error("Image error: {0}")]
  ImageError(#[from] image::ImageError),

  #[error("Failed to allocate device memory during initialization:\n    {0}")]
  AllocationError(#[from] GPUDataAllocationError),
  #[error("Failed to flush contents to host buffer memory")]
  HostMemorySyncError(#[from] HostMemorySyncError),

  #[error("Ran out of memory while issuing some command or creating memory: {0}")]
  GenericOutOfMemoryError(#[from] OutOfMemoryError),

  #[error("Failed to create swapchain:\n{0}")]
  SwapchainCreationFailed(#[from] SwapchainCreationError),

  #[error("Failed to create pipelines:\n{0}")]
  PipelineCreationFailed(#[from] PipelineCreationError),
  #[error("An error occurred when creating or saving the pipeline cache: {0}")]
  PipelineCacheError(#[from] PipelineCacheError),

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

impl From<HandleError> for InitializationError {
  fn from(value: HandleError) -> Self {
    InitializationError::WindowError(WindowError::HandleError(value))
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
  #[error("The compute thread has disconnected")]
  ComputeThreadDisconnected,

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
  #[error("Failed to sync screenshot buffer: {0}")]
  HostMemorySyncError(#[from] HostMemorySyncError),

  #[error("Image Error")]
  ImageError(#[from] image::ImageError),
}
impl std::fmt::Debug for ImageError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

#[derive(Debug, thiserror::Error)]
pub enum GPUDataAllocationError {
  #[error(transparent)]
  StagingBufferError(#[from] DeviceMemoryInitializationError),
  #[error("Failed to allocate one of the main device memory objects.\n{0}")]
  AllocationError(#[from] AllocationError),
  #[error("Failed to allocate one of the main host memory objects.\n{0}")]
  HostAllocationError(#[from] HostAllocationError),
  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),
  #[error("Failed to submit allocation workload to a queue: {0}")]
  QueueSubmitError(#[from] QueueSubmitError),
}
