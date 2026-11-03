use std::{
  ffi::{CStr, CString},
  path::Path,
  ptr,
};

use ash::vk;
use vkobjects::DeviceManuallyDestroyed;

use crate::{
  asset_loader::{ShaderLoadError, ShaderLoader},
  ENABLE_USE_DEBUG_SHADERS,
};

static MAIN_FN_NAME: &CStr = c"main";

const MAIN_VERT_SHADER_PATH: &str = "./assets/shaders/main_vert.spv";
const MAIN_VERT_DEBUG_SHADER_PATH: &str = "./assets/shaders/main_vert_debug.spv";
const MAIN_FRAG_SHADER_PATH: &str = "./assets/shaders/main_frag.spv";
const MAIN_FRAG_DEBUG_SHADER_PATH: &str = "./assets/shaders/main_frag_debug.spv";

const SKYBOX_VERT_SHADER_PATH: &str = "./assets/shaders/skybox_vert.spv";
const SKYBOX_VERT_DEBUG_SHADER_PATH: &str = "./assets/shaders/skybox_vert_debug.spv";
const SKYBOX_FRAG_SHADER_PATH: &str = "./assets/shaders/skybox_frag.spv";
const SKYBOX_FRAG_DEBUG_SHADER_PATH: &str = "./assets/shaders/skybox_frag_debug.spv";

const COLOR_VERT_SHADER_PATH: &str = "./assets/shaders/color_vert.spv";
const COLOR_VERT_DEBUG_SHADER_PATH: &str = "./assets/shaders/color_vert_debug.spv";
const COLOR_FRAG_SHADER_PATH: &str = "./assets/shaders/color_frag.spv";
const COLOR_FRAG_DEBUG_SHADER_PATH: &str = "./assets/shaders/color_frag_debug.spv";

const TEXT_VERT_SHADER_PATH: &str = "./assets/shaders/slug_vertex.spv";
const TEXT_VERT_DEBUG_SHADER_PATH: &str = "./assets/shaders/slug_vertex_debug.spv";
const TEXT_FRAG_SHADER_PATH: &str = "./assets/shaders/slug_pixel.spv";
const TEXT_FRAG_DEBUG_SHADER_PATH: &str = "./assets/shaders/slug_pixel_debug.spv";

#[derive(Debug, Clone, Copy)]
pub struct GraphicsShader {
  pub vert: vk::ShaderModule,
  pub frag: vk::ShaderModule,
}

impl GraphicsShader {
  pub fn load_vertex_frag(
    device: &ash::Device,
    shader_loader: &mut ShaderLoader,
    vertex_path: &str,
    vertex_debug_path: &str,
    fragment_path: &str,
    fragment_debug_path: &str,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
    #[cfg(feature = "vl")] name: &str,
  ) -> Result<Self, ShaderLoadError> {
    let vert_path = Path::new(if ENABLE_USE_DEBUG_SHADERS {
      vertex_debug_path
    } else {
      vertex_path
    });
    let frag_path = Path::new(if ENABLE_USE_DEBUG_SHADERS {
      fragment_debug_path
    } else {
      fragment_path
    });
    let vertex_name = CString::new(format!("{} Vertex Shader", name)).unwrap();
    let fragment_name = CString::new(format!("{} Fragment Shader", name)).unwrap();

    Ok(Self {
      vert: shader_loader.load_shader(
        device,
        vert_path,
        #[cfg(feature = "vl")]
        marker,
        #[cfg(feature = "vl")]
        &vertex_name,
      )?,
      frag: shader_loader.load_shader(
        device,
        frag_path,
        #[cfg(feature = "vl")]
        marker,
        #[cfg(feature = "vl")]
        &fragment_name,
      )?,
    })
  }

  pub fn get_pipeline_shader_creation_info(&self) -> [vk::PipelineShaderStageCreateInfo<'_>; 2] {
    [
      vk::PipelineShaderStageCreateInfo {
        // Vertex shader
        flags: vk::PipelineShaderStageCreateFlags::empty(),
        module: self.vert,
        p_name: MAIN_FN_NAME.as_ptr(),
        p_specialization_info: ptr::null(),
        stage: vk::ShaderStageFlags::VERTEX,
        ..Default::default()
      },
      vk::PipelineShaderStageCreateInfo {
        // Fragment shader
        flags: vk::PipelineShaderStageCreateFlags::empty(),
        module: self.frag,
        p_name: MAIN_FN_NAME.as_ptr(),
        p_specialization_info: ptr::null(),
        stage: vk::ShaderStageFlags::FRAGMENT,
        ..Default::default()
      },
    ]
  }

  pub fn load_main(
    device: &ash::Device,
    shader_loader: &mut ShaderLoader,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, ShaderLoadError> {
    Self::load_vertex_frag(
      device,
      shader_loader,
      MAIN_VERT_SHADER_PATH,
      MAIN_VERT_DEBUG_SHADER_PATH,
      MAIN_FRAG_SHADER_PATH,
      MAIN_FRAG_DEBUG_SHADER_PATH,
      #[cfg(feature = "vl")]
      marker,
      "Main",
    )
  }

  pub fn load_skybox(
    device: &ash::Device,
    shader_loader: &mut ShaderLoader,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, ShaderLoadError> {
    Self::load_vertex_frag(
      device,
      shader_loader,
      SKYBOX_VERT_SHADER_PATH,
      SKYBOX_VERT_DEBUG_SHADER_PATH,
      SKYBOX_FRAG_SHADER_PATH,
      SKYBOX_FRAG_DEBUG_SHADER_PATH,
      #[cfg(feature = "vl")]
      marker,
      "Skybox",
    )
  }

  pub fn load_color(
    device: &ash::Device,
    shader_loader: &mut ShaderLoader,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, ShaderLoadError> {
    Self::load_vertex_frag(
      device,
      shader_loader,
      COLOR_VERT_SHADER_PATH,
      COLOR_VERT_DEBUG_SHADER_PATH,
      COLOR_FRAG_SHADER_PATH,
      COLOR_FRAG_DEBUG_SHADER_PATH,
      #[cfg(feature = "vl")]
      marker,
      "Color",
    )
  }

  pub fn load_text(
    device: &ash::Device,
    shader_loader: &mut ShaderLoader,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, ShaderLoadError> {
    Self::load_vertex_frag(
      device,
      shader_loader,
      TEXT_VERT_SHADER_PATH,
      TEXT_VERT_DEBUG_SHADER_PATH,
      TEXT_FRAG_SHADER_PATH,
      TEXT_FRAG_DEBUG_SHADER_PATH,
      #[cfg(feature = "vl")]
      marker,
      "Text",
    )
  }
}

impl DeviceManuallyDestroyed for GraphicsShader {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_shader_module(self.vert, None);
    device.destroy_shader_module(self.frag, None);
  }
}
