mod cache;
mod compute;
mod graphics;

pub use cache::{create_pipeline_cache, save_pipeline_cache, PipelineCacheError};
pub use compute::{ComputePipeline, ComputePushConstants};
pub use graphics::{GraphicsPipeline, GraphicsPushConstants};
use vkobjects::errors::OutOfMemoryError;

use crate::render::shaders::ShaderError;

#[derive(Debug, thiserror::Error)]
pub enum PipelineCreationError {
  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),
  #[error("Failed to load shader")]
  ShaderFailed(#[source] ShaderError),
  #[error("Failed to compile or link shaders")]
  CompilationFailed,
}

impl From<OutOfMemoryError> for PipelineCreationError {
  fn from(value: OutOfMemoryError) -> Self {
    PipelineCreationError::OutOfMemory(value)
  }
}
