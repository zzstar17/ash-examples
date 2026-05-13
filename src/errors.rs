use vkallocator::{AllocationError, DeviceMemoryInitializationError};
use vkinitialization::{
  device::{device_selector::PhysicalDeviceSelectionError, DeviceCreationError},
  InstanceCreationError,
};
use vkobjects::errors::{OutOfMemoryError, QueueSubmitError};

use crate::pipelines::{PipelineCacheError, PipelineCreationError};

#[derive(thiserror::Error, Debug)]
pub enum InitializationError {
  #[error("Instance creation failed:\n    {0}")]
  InstanceCreationFailed(#[from] InstanceCreationError),

  #[error("An error occurred during device selection: {0}")]
  DeviceSelectionError(#[from] PhysicalDeviceSelectionError),
  #[error("No physical device supports the application")]
  NoCompatibleDevices,
  #[error("An error occurred during the creation of the logical device:\n    {0}")]
  DeviceCreationError(#[from] DeviceCreationError),

  #[error("Some command failed because of a generic OutOfMemory error: {0}")]
  OutOfMemoryError(#[from] OutOfMemoryError),
  #[error("Failed to allocate device memory:\n    ")]
  AllocationError(#[from] DeviceMemoryInitializationError),
  #[error("Failed to submit some queue: {0}")]
  QueueSubmissionError(#[from] QueueSubmitError),
  #[error("Failed to create pipelines:\n{0}")]
  PipelineCreationFailed(#[from] PipelineCreationError),
  #[error("An error occurred when creating or saving the pipeline cache: {0}")]
  PipelineCacheError(#[from] PipelineCacheError),

  #[error(transparent)]
  IOError(#[from] std::io::Error),
}

impl From<AllocationError> for InitializationError {
  fn from(value: AllocationError) -> Self {
    Self::AllocationError(value.into())
  }
}
