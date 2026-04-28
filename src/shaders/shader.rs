use std::{ffi::CStr, marker::PhantomData, path::Path, ptr};

use ash::vk;

use crate::device_destroyable::DeviceManuallyDestroyed;

use super::{load_shader, ShaderError};

const VERT_SHADER_PATH: &str = "./shaders/vert.spv";
const FRAG_SHADER_PATH: &str = "./shaders/frag.spv";

static MAIN_FN_NAME: &CStr = c"main";

pub struct Shader {
  pub vert: vk::ShaderModule,
  pub frag: vk::ShaderModule,
}

impl Shader {
  pub fn load(device: &ash::Device) -> Result<Self, ShaderError> {
    Ok(Self {
      vert: load_shader(device, Path::new(VERT_SHADER_PATH))?,
      frag: load_shader(device, Path::new(FRAG_SHADER_PATH))?,
    })
  }
}

impl Shader {
  pub fn get_pipeline_shader_creation_info(&self) -> [vk::PipelineShaderStageCreateInfo<'_>; 2] {
    [
      vk::PipelineShaderStageCreateInfo {
        // Vertex shader
        s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
        p_next: ptr::null(),
        flags: vk::PipelineShaderStageCreateFlags::empty(),
        module: self.vert,
        p_name: MAIN_FN_NAME.as_ptr(),
        p_specialization_info: ptr::null(),
        stage: vk::ShaderStageFlags::VERTEX,
        _marker: PhantomData,
      },
      vk::PipelineShaderStageCreateInfo {
        // Fragment shader
        s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
        p_next: ptr::null(),
        flags: vk::PipelineShaderStageCreateFlags::empty(),
        module: self.frag,
        p_name: MAIN_FN_NAME.as_ptr(),
        p_specialization_info: ptr::null(),
        stage: vk::ShaderStageFlags::FRAGMENT,
        _marker: PhantomData,
      },
    ]
  }
}

impl DeviceManuallyDestroyed for Shader {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_shader_module(self.vert, None);
    device.destroy_shader_module(self.frag, None);
  }
}
