use std::{
  fs::File,
  io::{self, Read},
  path::Path,
};

use ash::vk;
use vkobjects::errors::OutOfMemoryError;

pub struct ShaderLoader {
  shader_buffer: Vec<u8>,
}

#[derive(thiserror::Error, Debug)]
pub enum ShaderLoadError {
  #[error("IO error on path {1}: {0}")]
  IOError(#[source] io::Error, String),

  #[error("Failed to compile or link")]
  Invalid,

  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),
}

impl ShaderLoader {
  pub fn new() -> Self {
    Self {
      shader_buffer: Vec::new(),
    }
  }

  pub fn load_shader(
    &mut self,
    device: &ash::Device,
    shader_path: &Path,
  ) -> Result<vk::ShaderModule, ShaderLoadError> {
    self
      .read_shader_code(shader_path)
      .map_err(|err| ShaderLoadError::IOError(err, format!("{:?}", shader_path)))?;

    log::debug!(
      "Creating shader module with len {} from path {:?}",
      self.shader_buffer.len(),
      shader_path
    );
    if !self.shader_buffer.len().is_multiple_of(4) || self.shader_buffer.is_empty() {
      return Err(ShaderLoadError::Invalid);
    }

    let module = self.create_shader_module(device)?;
    Ok(module)
  }

  fn read_shader_code(&mut self, shader_path: &Path) -> io::Result<()> {
    let mut file = File::open(shader_path)?;
    self.shader_buffer.clear();
    file.read_to_end(&mut self.shader_buffer)?;
    Ok(())
  }

  fn create_shader_module(
    &self,
    device: &ash::Device,
  ) -> Result<vk::ShaderModule, ShaderLoadError> {
    let create_info = vk::ShaderModuleCreateInfo {
      p_code: self.shader_buffer.as_ptr() as *const u32,
      code_size: self.shader_buffer.len(), // bytes
      ..Default::default()
    };

    unsafe { device.create_shader_module(&create_info, None) }.map_err(|vkerr| match vkerr {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        ShaderLoadError::OutOfMemory(vkerr.into())
      }
      vk::Result::ERROR_INVALID_SHADER_NV => ShaderLoadError::Invalid,
      _ => panic!(),
    })
  }
}
