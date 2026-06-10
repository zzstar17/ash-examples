use std::{ffi::CStr, marker::PhantomData, path::Path, ptr};

use ash::vk;
use vkobjects::DeviceManuallyDestroyed;

use super::{load_shader, ShaderError};

const PATH: &str = "./shaders/compute/shader.spv";

static MAIN_FN_NAME: &CStr = c"main";

#[derive(Debug, Clone, Copy)]
pub struct Shader {
  pub module: vk::ShaderModule,
}

impl Shader {
  pub fn load(device: &ash::Device) -> Result<Self, ShaderError> {
    Ok(Self {
      module: load_shader(device, Path::new(PATH))?,
    })
  }
}

impl Shader {
  pub fn get_pipeline_shader_creation_info(&self) -> vk::PipelineShaderStageCreateInfo<'_> {
    vk::PipelineShaderStageCreateInfo {
      s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineShaderStageCreateFlags::empty(),
      module: self.module,
      p_name: MAIN_FN_NAME.as_ptr(),
      p_specialization_info: ptr::null(),
      stage: vk::ShaderStageFlags::COMPUTE,
      _marker: PhantomData,
    }
  }
}

impl DeviceManuallyDestroyed for Shader {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_shader_module(self.module, None);
  }
}
