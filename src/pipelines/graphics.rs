use std::{
  marker::PhantomData,
  ptr::{self, addr_of},
};

use ash::vk;

use crate::{
  shaders::{self},
  vertex_input_state_create_info,
  vertices::Vertex,
  IMAGE_HEIGHT, IMAGE_WIDTH,
};
use vkobjects::{errors::OutOfMemoryError, DeviceManuallyDestroyed};

use super::PipelineCreationError;

pub struct GraphicsPipeline {
  pub layout: vk::PipelineLayout,
  pub pipeline: vk::Pipeline,
}

impl GraphicsPipeline {
  pub fn create(
    device: &ash::Device,
    cache: vk::PipelineCache,
    render_pass: vk::RenderPass,
  ) -> Result<Self, PipelineCreationError> {
    let shader = shaders::Shader::load(device).map_err(PipelineCreationError::ShaderFailed)?;
    let shader_stages = shader.get_pipeline_shader_creation_info();

    let vertex_input_state = vertex_input_state_create_info!(Vertex);

    let input_assembly_state_ci = triangle_input_assembly_state();

    // full image viewport and scissor
    let viewport = vk::Viewport {
      x: 0.0,
      y: 0.0,
      width: IMAGE_WIDTH as f32,
      height: IMAGE_HEIGHT as f32,
      min_depth: 0.0,
      max_depth: 1.0,
    };
    let scissor = vk::Rect2D {
      offset: vk::Offset2D { x: 0, y: 0 },
      extent: vk::Extent2D {
        width: IMAGE_WIDTH,
        height: IMAGE_HEIGHT,
      },
    };
    let viewport_state = vk::PipelineViewportStateCreateInfo {
      s_type: vk::StructureType::PIPELINE_VIEWPORT_STATE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineViewportStateCreateFlags::empty(),
      scissor_count: 1,
      p_scissors: addr_of!(scissor),
      viewport_count: 1,
      p_viewports: addr_of!(viewport),
      _marker: PhantomData,
    };

    let rasterization_state_ci = no_depth_rasterization_state();
    let multisample_state_ci = no_multisample_state();

    let attachment_state = vk::PipelineColorBlendAttachmentState {
      // no blend state
      blend_enable: vk::FALSE,
      color_write_mask: vk::ColorComponentFlags::RGBA,

      // everything else doesn't matter
      ..Default::default()
    };
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
      s_type: vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineColorBlendStateCreateFlags::empty(),
      logic_op_enable: vk::FALSE,
      logic_op: vk::LogicOp::COPY, // disabled
      attachment_count: 1,
      p_attachments: addr_of!(attachment_state),
      blend_constants: [0.0, 0.0, 0.0, 0.0],
      _marker: PhantomData,
    };

    // no descriptor sets or push constants
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: 0,
      p_set_layouts: ptr::null(),
      push_constant_range_count: 0,
      p_push_constant_ranges: ptr::null(),
      _marker: PhantomData,
    };
    let layout = unsafe { device.create_pipeline_layout(&layout_create_info, None) }
      .map_err(OutOfMemoryError::from)?;

    let create_info = vk::GraphicsPipelineCreateInfo {
      s_type: vk::StructureType::GRAPHICS_PIPELINE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineCreateFlags::empty(),
      stage_count: shader_stages.len() as u32,
      p_stages: shader_stages.as_ptr(),
      p_vertex_input_state: vertex_input_state.get(),
      p_input_assembly_state: &input_assembly_state_ci,
      p_tessellation_state: ptr::null(),
      p_viewport_state: &viewport_state,
      p_rasterization_state: &rasterization_state_ci,
      p_multisample_state: &multisample_state_ci,
      p_depth_stencil_state: ptr::null(),
      p_color_blend_state: &color_blend_state,
      p_dynamic_state: ptr::null(),
      layout,
      render_pass,
      subpass: 0,
      base_pipeline_handle: vk::Pipeline::null(),
      base_pipeline_index: -1, // -1 for null
      _marker: PhantomData,
    };
    let pipeline = unsafe {
      device
        .create_graphics_pipelines(cache, &[create_info], None)
        .map_err(|incomplete| incomplete.1)
        .map_err(|vkerr| match vkerr {
          vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
            PipelineCreationError::from(OutOfMemoryError::from(vkerr))
          }
          vk::Result::ERROR_INVALID_SHADER_NV => PipelineCreationError::CompilationFailed,
          _ => panic!(),
        })?[0]
    };

    unsafe {
      shader.destroy_self(device);
    }

    Ok(Self { layout, pipeline })
  }
}

impl DeviceManuallyDestroyed for GraphicsPipeline {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_pipeline(self.pipeline, None);
    device.destroy_pipeline_layout(self.layout, None);
  }
}

fn triangle_input_assembly_state<'a>() -> vk::PipelineInputAssemblyStateCreateInfo<'a> {
  vk::PipelineInputAssemblyStateCreateInfo {
    s_type: vk::StructureType::PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
    flags: vk::PipelineInputAssemblyStateCreateFlags::empty(),
    p_next: ptr::null(),
    // defines that there exists a special value that restarts the assembly
    primitive_restart_enable: vk::FALSE,
    topology: vk::PrimitiveTopology::TRIANGLE_LIST,
    _marker: PhantomData,
  }
}

// rasterization with no depth and no culling
fn no_depth_rasterization_state<'a>() -> vk::PipelineRasterizationStateCreateInfo<'a> {
  vk::PipelineRasterizationStateCreateInfo {
    s_type: vk::StructureType::PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::PipelineRasterizationStateCreateFlags::empty(),
    depth_clamp_enable: vk::FALSE,
    cull_mode: vk::CullModeFlags::NONE,
    front_face: vk::FrontFace::CLOCKWISE, // doesn't matter if cull_mode is none
    line_width: 1.0,
    polygon_mode: vk::PolygonMode::FILL,
    rasterizer_discard_enable: vk::FALSE,
    depth_bias_clamp: 0.0,
    depth_bias_constant_factor: 0.0,
    depth_bias_enable: vk::FALSE,
    depth_bias_slope_factor: 0.0,
    _marker: PhantomData,
  }
}

fn no_multisample_state<'a>() -> vk::PipelineMultisampleStateCreateInfo<'a> {
  // everything off
  vk::PipelineMultisampleStateCreateInfo {
    s_type: vk::StructureType::PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
    flags: vk::PipelineMultisampleStateCreateFlags::empty(),
    p_next: ptr::null(),
    rasterization_samples: vk::SampleCountFlags::TYPE_1,
    sample_shading_enable: vk::FALSE,
    min_sample_shading: 0.0,
    p_sample_mask: ptr::null(),
    alpha_to_one_enable: vk::FALSE,
    alpha_to_coverage_enable: vk::FALSE,
    _marker: PhantomData,
  }
}
