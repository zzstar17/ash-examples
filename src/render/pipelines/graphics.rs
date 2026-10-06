use std::{
  ffi::c_void,
  marker::PhantomData,
  mem::{self, size_of},
  ops::BitOr,
  ptr::{self, addr_of},
};

use ash::vk::{self, Handle};
use ash_slug::{SlugPushConstants, SlugVertex};
use cgmath::Matrix4;

use crate::{
  asset_loader::ShaderLoader,
  render::{
    descriptor_sets::DescriptorPool,
    render_targets::DEPTH_FORMAT,
    sample_count_to_flags,
    shaders::{self, Shader, TextShader},
    TexturedVertex,
  },
  vertex_input_state_create_info,
};
use vkobjects::{errors::OutOfMemoryError, utility::OnErr, DeviceManuallyDestroyed};

use super::PipelineCreationError;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GraphicsPushConstants {
  pub matrix: Matrix4<f32>,
  pub tex_offset: [f32; 2],
  pub tex_size: [f32; 2],
}

impl Default for GraphicsPushConstants {
  fn default() -> Self {
    Self {
      matrix: Matrix4::new(
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
      ),
      tex_offset: [0.0, 0.0],
      tex_size: [0.0, 0.0],
    }
  }
}

pub struct RenderPipelines {
  pub graphics: GraphicsPipeline,
  pub text: TextPipeline,
}

pub struct GraphicsPipeline {
  pub layout: vk::PipelineLayout,
  pub current: vk::Pipeline,

  pub shader: Shader,
  pub old: Option<vk::Pipeline>,
}

pub struct TextPipeline {
  pub layout: vk::PipelineLayout,
  pub current: vk::Pipeline,

  shader: TextShader,
  old: Option<vk::Pipeline>,
}

impl RenderPipelines {
  pub fn new(
    device: &ash::Device,
    cache: vk::PipelineCache,
    shader_loader: &mut ShaderLoader,
    descriptor_pool: &DescriptorPool,
    render_format: vk::Format,
    extent: vk::Extent2D,
    sample_count: usize,
  ) -> Result<Self, PipelineCreationError> {
    let graphics_layout = Self::create_graphics_layout(device, descriptor_pool)?;
    let text_layout = Self::create_text_layout(device, descriptor_pool).on_err(|_err| unsafe {
      device.destroy_pipeline_layout(graphics_layout, None);
    })?;

    let graphics_shader = shaders::Shader::load(device, shader_loader)
      .map_err(PipelineCreationError::ShaderFailed)
      .on_err(|_err| unsafe {
        device.destroy_pipeline_layout(graphics_layout, None);
        device.destroy_pipeline_layout(text_layout, None);
      })?;

    let text_shader = shaders::TextShader::load(device, shader_loader)
      .map_err(PipelineCreationError::ShaderFailed)
      .on_err(|_err| unsafe {
        device.destroy_pipeline_layout(graphics_layout, None);
        device.destroy_pipeline_layout(text_layout, None);
        graphics_shader.destroy_self(device);
      })?;

    let [initial_graphics, initial_text] = Self::create_with_base(
      device,
      graphics_layout,
      text_layout,
      &graphics_shader,
      &text_shader,
      cache,
      vk::Pipeline::null(),
      vk::Pipeline::null(),
      render_format,
      extent,
      sample_count,
    )?;

    let graphics = GraphicsPipeline {
      layout: graphics_layout,
      current: initial_graphics,
      shader: graphics_shader,
      old: None,
    };
    let text = TextPipeline {
      layout: text_layout,
      current: initial_text,
      shader: text_shader,
      old: None,
    };

    Ok(Self { graphics, text })
  }

  // create a new pipeline and mark the other as old
  pub fn recreate(
    &mut self,
    device: &ash::Device,
    cache: vk::PipelineCache,
    render_format: vk::Format,
    extent: vk::Extent2D,
    sample_count: usize,
  ) -> Result<(), PipelineCreationError> {
    assert!(self.graphics.old.is_none());
    assert!(self.text.old.is_none());

    let mut new = Self::create_with_base(
      device,
      self.graphics.layout,
      self.text.layout,
      &self.graphics.shader,
      &self.text.shader,
      cache,
      self.graphics.current,
      self.text.current,
      render_format,
      extent,
      sample_count,
    )?;

    let old_graphics = {
      mem::swap(&mut self.graphics.current, &mut new[0]);
      new[0]
    };
    let old_text = {
      mem::swap(&mut self.text.current, &mut new[1]);
      new[1]
    };

    self.graphics.old = Some(old_graphics);
    self.text.old = Some(old_text);
    Ok(())
  }

  #[allow(dead_code)]
  pub fn revert_recreate(&mut self, device: &ash::Device) {
    unsafe {
      self.graphics.current.destroy_self(device);
      self.text.current.destroy_self(device);
    }
    let mut temp = None;
    mem::swap(&mut self.graphics.old, &mut temp);
    self.graphics.current = temp.unwrap();
    let mut temp = None;
    mem::swap(&mut self.text.old, &mut temp);
    self.text.current = temp.unwrap();
  }

  // destroy old pipeline once it stops being used
  pub unsafe fn destroy_old(&mut self, device: &ash::Device, cur_total_frame: usize) {
    if let Some(graphics_old) = self.graphics.old {
      log::debug!("[Frame {}] Destroying old pipelines", cur_total_frame);
      let text_old = self.text.old.unwrap();
      device.destroy_pipeline(graphics_old, None);
      device.destroy_pipeline(text_old, None);
      self.graphics.old = None;
      self.text.old = None;
    }
  }

  fn create_graphics_layout(
    device: &ash::Device,
    descriptor_pool: &DescriptorPool,
  ) -> Result<vk::PipelineLayout, OutOfMemoryError> {
    let push_constant_range = vk::PushConstantRange {
      stage_flags: vk::ShaderStageFlags::VERTEX,
      offset: 0,
      size: size_of::<GraphicsPushConstants>() as u32,
    };
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: 1,
      p_set_layouts: &descriptor_pool.sprites_layout,
      push_constant_range_count: 1,
      p_push_constant_ranges: &push_constant_range,
      ..Default::default()
    };
    unsafe { device.create_pipeline_layout(&layout_create_info, None) }
      .map_err(OutOfMemoryError::from)
  }

  fn create_text_layout(
    device: &ash::Device,
    descriptor_pool: &DescriptorPool,
  ) -> Result<vk::PipelineLayout, OutOfMemoryError> {
    let push_constant_range = vk::PushConstantRange {
      stage_flags: vk::ShaderStageFlags::VERTEX,
      offset: 0,
      size: size_of::<SlugPushConstants>() as u32,
    };
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      set_layout_count: 1,
      p_set_layouts: &descriptor_pool.text_layout,
      push_constant_range_count: 1,
      p_push_constant_ranges: &push_constant_range,
      ..Default::default()
    };
    unsafe { device.create_pipeline_layout(&layout_create_info, None) }
      .map_err(OutOfMemoryError::from)
  }

  fn create_with_base(
    device: &ash::Device,
    graphics_layout: vk::PipelineLayout,
    text_layout: vk::PipelineLayout,
    graphics_shader: &Shader,
    text_shader: &TextShader,
    cache: vk::PipelineCache,
    graphics_base: vk::Pipeline,
    text_base: vk::Pipeline,
    render_format: vk::Format,
    extent: vk::Extent2D,
    sample_count: usize,
  ) -> Result<[vk::Pipeline; 2], PipelineCreationError> {
    let graphics_shader_stages = graphics_shader.get_pipeline_shader_creation_info();
    let text_shader_stages = text_shader.get_pipeline_shader_creation_info();

    let graphics_vertex_input_state = vertex_input_state_create_info!(TexturedVertex);
    let graphics_vertex_input_state = graphics_vertex_input_state.get();
    let text_vertex_input_state = vertex_input_state_create_info!(SlugVertex);
    let text_vertex_input_state = text_vertex_input_state.get();

    let input_assembly_state = triangle_input_assembly_state();

    // full image viewport and scissor
    let viewport = [vk::Viewport {
      x: 0.0,
      y: 0.0,
      width: extent.width as f32,
      height: extent.height as f32,
      min_depth: 0.0,
      max_depth: 1.0,
    }];
    let scissor = [vk::Rect2D {
      offset: vk::Offset2D { x: 0, y: 0 },
      extent: vk::Extent2D {
        width: extent.width,
        height: extent.height,
      },
    }];
    let viewport_state = vk::PipelineViewportStateCreateInfo::default()
      .scissors(&scissor)
      .viewports(&viewport);

    let graphics_rasterization_state_ci = vk::PipelineRasterizationStateCreateInfo {
      flags: vk::PipelineRasterizationStateCreateFlags::empty(),
      depth_clamp_enable: vk::FALSE,
      cull_mode: vk::CullModeFlags::BACK,
      front_face: vk::FrontFace::CLOCKWISE,
      line_width: 1.0,
      polygon_mode: vk::PolygonMode::FILL,
      rasterizer_discard_enable: vk::FALSE,
      depth_bias_clamp: 0.0,
      depth_bias_constant_factor: 0.0,
      depth_bias_enable: vk::FALSE,
      depth_bias_slope_factor: 0.0,
      ..Default::default()
    };
    let text_rasterization_state_ci = vk::PipelineRasterizationStateCreateInfo {
      cull_mode: vk::CullModeFlags::BACK,
      front_face: vk::FrontFace::COUNTER_CLOCKWISE,
      ..graphics_rasterization_state_ci
    };

    let multisample_state_ci = vk::PipelineMultisampleStateCreateInfo {
      rasterization_samples: sample_count_to_flags(sample_count),
      sample_shading_enable: vk::FALSE,
      min_sample_shading: 0.0,
      p_sample_mask: ptr::null(),
      alpha_to_one_enable: vk::FALSE,
      alpha_to_coverage_enable: vk::FALSE,
      ..Default::default()
    };

    let depth_stencil_state_ci = vk::PipelineDepthStencilStateCreateInfo {
      flags: vk::PipelineDepthStencilStateCreateFlags::empty(),
      depth_test_enable: vk::TRUE,
      depth_write_enable: vk::TRUE,
      depth_compare_op: vk::CompareOp::LESS,
      depth_bounds_test_enable: vk::FALSE,
      min_depth_bounds: 0.0,
      max_depth_bounds: 1.0,
      stencil_test_enable: vk::FALSE,
      ..Default::default()
    };

    let attachment_state = vk::PipelineColorBlendAttachmentState {
      // blend by opacity
      blend_enable: vk::TRUE,
      color_write_mask: vk::ColorComponentFlags::RGBA,

      // final_color = (src_alpha * src_color) + ((1 - src_alpha) * dst_color)
      src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
      dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
      color_blend_op: vk::BlendOp::ADD,

      // always set alpha to one
      src_alpha_blend_factor: vk::BlendFactor::ONE,
      dst_alpha_blend_factor: vk::BlendFactor::ONE,
      alpha_blend_op: vk::BlendOp::ADD,
    };
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
      s_type: vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineColorBlendStateCreateFlags::empty(),
      logic_op_enable: vk::FALSE,
      logic_op: vk::LogicOp::COPY, // disabled
      attachment_count: 1,
      p_attachments: &attachment_state,
      blend_constants: [0.0, 0.0, 0.0, 0.0],
      _marker: PhantomData,
    };

    let render_formats = [render_format];
    let rendering_create_info = vk::PipelineRenderingCreateInfo {
      color_attachment_count: render_formats.len() as u32,
      p_color_attachment_formats: render_formats.as_ptr(),
      depth_attachment_format: DEPTH_FORMAT,
      stencil_attachment_format: vk::Format::UNDEFINED,
      ..Default::default()
    };

    let dynamic = [vk::DynamicState::DEPTH_TEST_ENABLE];
    // let dynamic = [];
    let dynamic_state_ci = vk::PipelineDynamicStateCreateInfo {
      flags: vk::PipelineDynamicStateCreateFlags::empty(),
      dynamic_state_count: dynamic.len() as u32,
      p_dynamic_states: dynamic.as_ptr(),
      ..Default::default()
    };

    let mut graphics_flags = vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
    let mut text_flags = vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
    if !graphics_base.is_null() {
      graphics_flags = graphics_flags.bitor(vk::PipelineCreateFlags::DERIVATIVE);
    }
    if !text_base.is_null() {
      text_flags = text_flags.bitor(vk::PipelineCreateFlags::DERIVATIVE);
    }
    let graphics_create_info = vk::GraphicsPipelineCreateInfo {
      p_next: addr_of!(rendering_create_info) as *const c_void,
      flags: graphics_flags,
      stage_count: graphics_shader_stages.len() as u32,
      p_stages: graphics_shader_stages.as_ptr(),
      p_vertex_input_state: graphics_vertex_input_state,
      p_input_assembly_state: &input_assembly_state,
      p_tessellation_state: ptr::null(),
      p_viewport_state: &viewport_state,
      p_rasterization_state: &graphics_rasterization_state_ci,
      p_multisample_state: &multisample_state_ci,
      p_depth_stencil_state: &depth_stencil_state_ci,
      p_color_blend_state: &color_blend_state,
      p_dynamic_state: &dynamic_state_ci,
      layout: graphics_layout,
      render_pass: vk::RenderPass::null(), // replaced by dynamic rendering
      subpass: 0,
      base_pipeline_handle: graphics_base,
      base_pipeline_index: -1, // -1 for null
      ..Default::default()
    };
    let text_create_info = vk::GraphicsPipelineCreateInfo {
      flags: text_flags,
      stage_count: text_shader_stages.len() as u32,
      p_stages: text_shader_stages.as_ptr(),
      p_vertex_input_state: text_vertex_input_state,
      p_rasterization_state: &text_rasterization_state_ci,
      layout: text_layout,
      base_pipeline_handle: text_base,
      base_pipeline_index: -1, // -1 for null
      ..graphics_create_info
    };
    let create_infos = [graphics_create_info, text_create_info];

    let pipelines = unsafe {
      device
        .create_graphics_pipelines(cache, &create_infos, None)
        .map_err(|incomplete| incomplete.1)
        .map_err(|vkerr| match vkerr {
          vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
            PipelineCreationError::from(OutOfMemoryError::from(vkerr))
          }
          vk::Result::ERROR_INVALID_SHADER_NV => PipelineCreationError::CompilationFailed,
          _ => panic!(),
        })
    }?;
    let graphics = pipelines[0];
    let text = pipelines[1];

    Ok([graphics, text])
  }
}

const fn triangle_input_assembly_state<'a>() -> vk::PipelineInputAssemblyStateCreateInfo<'a> {
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

impl DeviceManuallyDestroyed for RenderPipelines {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.graphics.destroy_self(device);
    self.text.destroy_self(device);
  }
}

impl DeviceManuallyDestroyed for GraphicsPipeline {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    if let Some(old) = self.old {
      device.destroy_pipeline(old, None);
    }
    device.destroy_pipeline(self.current, None);
    device.destroy_pipeline_layout(self.layout, None);

    // can be unloaded any time
    self.shader.destroy_self(device);
  }
}

impl DeviceManuallyDestroyed for TextPipeline {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    if let Some(old) = self.old {
      device.destroy_pipeline(old, None);
    }
    device.destroy_pipeline(self.current, None);
    device.destroy_pipeline_layout(self.layout, None);

    // can be unloaded any time
    self.shader.destroy_self(device);
  }
}
