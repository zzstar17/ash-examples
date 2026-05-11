use crate::shaders::ShaderError;

mod cache;
mod graphics;

pub use cache::{create_pipeline_cache, save_pipeline_cache, PipelineCacheError};
pub use graphics::GraphicsPipeline;
use vkobjects::errors::OutOfMemoryError;

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
