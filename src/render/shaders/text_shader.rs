use std::{ffi::CStr, marker::PhantomData, path::Path, ptr};

use ash::vk;
use vkobjects::DeviceManuallyDestroyed;

use crate::ENABLE_USE_DEBUG_SHADERS;

use super::{load_shader, ShaderError};

const VERT_SHADER_PATH: &str = "./shaders/slug_vertex.spv";
const VERT_DEBUG_SHADER_PATH: &str = "./shaders/slug_vertex_debug.spv";
const FRAG_SHADER_PATH: &str = "./shaders/slug_pixel.spv";
const FRAG_DEBUG_SHADER_PATH: &str = "./shaders/slug_pixel_debug.spv";

static MAIN_FN_NAME: &CStr = c"main";

pub struct TextShader {
  pub vert: vk::ShaderModule,
  pub frag: vk::ShaderModule,
}

impl TextShader {
  pub fn load(device: &ash::Device) -> Result<Self, ShaderError> {
    let vert_path = Path::new(if ENABLE_USE_DEBUG_SHADERS {
      VERT_DEBUG_SHADER_PATH
    } else {
      VERT_SHADER_PATH
    });
    let frag_path = Path::new(if ENABLE_USE_DEBUG_SHADERS {
      FRAG_DEBUG_SHADER_PATH
    } else {
      FRAG_SHADER_PATH
    });
    Ok(Self {
      vert: load_shader(device, vert_path)?,
      frag: load_shader(device, frag_path)?,
    })
  }
}

impl TextShader {
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

impl DeviceManuallyDestroyed for TextShader {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_shader_module(self.vert, None);
    device.destroy_shader_module(self.frag, None);
  }
}
