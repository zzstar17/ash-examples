mod cache;
mod graphics;
mod text;

pub use cache::{create_pipeline_cache, save_pipeline_cache, PipelineCacheError};
pub use graphics::{GraphicsPipeline, GraphicsPushConstants};
pub use text::TextPipeline;
use vkobjects::errors::OutOfMemoryError;

use crate::asset_loader::ShaderLoadError;

#[derive(Debug, thiserror::Error)]
pub enum PipelineCreationError {
  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),
  #[error("Failed to load shader\n{0}")]
  ShaderFailed(#[from] ShaderLoadError),
  #[error("Failed to compile or link shaders")]
  CompilationFailed,
}

impl From<OutOfMemoryError> for PipelineCreationError {
  fn from(value: OutOfMemoryError) -> Self {
    PipelineCreationError::OutOfMemory(value)
  }
}
