use std::{marker::PhantomData, mem::size_of, ptr};

use ash::vk;
use vkobjects::{errors::OutOfMemoryError, DeviceManuallyDestroyed};

use crate::render::{descriptor_sets::ComputeDescriptorPool, shaders};

use super::PipelineCreationError;

// equivalent to src/render/shaders/compute/shader.comp
#[repr(C)]
#[derive(Debug, Default)]
pub struct ComputePushConstants {
  pub render_dimensions: [f32; 2],
  pub player_pos: [f32; 2],

  pub particle_count: u32,
  pub new_particle_count: u32,
}

#[derive(Debug)]
pub struct ComputePipeline {
  pub layout: vk::PipelineLayout,
  pub main: vk::Pipeline,
}

impl ComputePipeline {
  pub fn new(
    device: &ash::Device,
    cache: vk::PipelineCache,
    descriptor_pool: &ComputeDescriptorPool,
  ) -> Result<Self, PipelineCreationError> {
    let layout = Self::create_layout(device, descriptor_pool)?;

    let shader =
      shaders::compute::Shader::load(device).map_err(PipelineCreationError::ShaderFailed)?;
    let shader_stages = shader.get_pipeline_shader_creation_info();

    let create_info = vk::ComputePipelineCreateInfo {
      s_type: vk::StructureType::COMPUTE_PIPELINE_CREATE_INFO,
      p_next: ptr::null(),
      stage: shader_stages,
      flags: vk::PipelineCreateFlags::empty(),
      layout,
      base_pipeline_handle: vk::Pipeline::null(),
      base_pipeline_index: -1, // -1 for invalid
      _marker: PhantomData,
    };
    let main = unsafe { device.create_compute_pipelines(cache, &[create_info], None) }
      .map_err(|incomplete| incomplete.1)
      .map_err(|vkerr| match vkerr {
        vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
          PipelineCreationError::from(OutOfMemoryError::from(vkerr))
        }
        vk::Result::ERROR_INVALID_SHADER_NV => PipelineCreationError::CompilationFailed,
        _ => panic!(),
      })?[0];

    unsafe {
      shader.destroy_self(device);
    }

    Ok(Self { layout, main })
  }

  fn create_layout(
    device: &ash::Device,
    descriptor_pool: &ComputeDescriptorPool,
  ) -> Result<vk::PipelineLayout, OutOfMemoryError> {
    let push_constant_range = vk::PushConstantRange {
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      offset: 0,
      size: size_of::<ComputePushConstants>() as u32,
    };
    let descriptor_layouts = [descriptor_pool.single_storage_buffer_layout; 3];
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: descriptor_layouts.len() as u32,
      p_set_layouts: descriptor_layouts.as_ptr(),
      push_constant_range_count: 1,
      p_push_constant_ranges: &push_constant_range,
      _marker: PhantomData,
    };
    unsafe { device.create_pipeline_layout(&layout_create_info, None) }
      .map_err(OutOfMemoryError::from)
  }
}

impl DeviceManuallyDestroyed for ComputePipeline {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.main.destroy_self(device);
    device.destroy_pipeline_layout(self.layout, None);
  }
}
