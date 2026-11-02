pub mod model_loader;
mod shader_loader;
pub mod texture_loader;

pub use model_loader::{LoadedModels, Mesh};
pub use shader_loader::{ShaderLoadError, ShaderLoader};

use crate::asset_loader::{model_loader::ModelGpuData, texture_loader::LoadedSkybox};

pub struct LoadedData {
  pub model: ModelGpuData,
  pub skybox: LoadedSkybox,
}
