#[cfg(feature = "vl")]
use std::ffi::CStr;
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
    descriptor_sets::DescriptorPool, render_targets::DEPTH_FORMAT, sample_count_to_flags,
    shaders::GraphicsShader, TexturedVertex,
  },
  vertex_input_state_create_info,
};
use vkobjects::{errors::OutOfMemoryError, utility::OnErr, DeviceManuallyDestroyed};

use super::PipelineCreationError;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GraphicsPushConstants {
  pub matrix: Matrix4<f32>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ColorPushConstants {
  pub matrix: Matrix4<f32>,
  pub color: [f32; 4],
}

impl Default for GraphicsPushConstants {
  fn default() -> Self {
    Self {
      matrix: Matrix4::new(
        0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
      ),
    }
  }
}

pub struct RenderPipelines {
  pub main: Pipeline,
  pub skybox: Pipeline,
  pub color: Pipeline,
  pub text: Pipeline,
}

struct Layouts {
  pub main: vk::PipelineLayout,
  pub skybox: vk::PipelineLayout,
  pub color: vk::PipelineLayout,
  pub text: vk::PipelineLayout,
}

struct Shaders {
  pub main: GraphicsShader,
  pub skybox: GraphicsShader,
  pub color: GraphicsShader,
  pub text: GraphicsShader,
}

struct PipelineHandles {
  pub main: vk::Pipeline,
  pub skybox: vk::Pipeline,
  pub color: vk::Pipeline,
  pub text: vk::Pipeline,
}

pub struct Pipeline {
  pub layout: vk::PipelineLayout,
  pub current: vk::Pipeline,

  pub shader: GraphicsShader,
  pub old: Option<vk::Pipeline>,
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
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, PipelineCreationError> {
    let main_layout = Self::create_graphics_layout(
      device,
      descriptor_pool,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Graphics layout",
    )?;
    let skybox_layout = Self::create_skybox_layout(
      device,
      descriptor_pool,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Skybox layout",
    )
    .on_err(|_err| unsafe {
      main_layout.destroy_self(device);
    })?;
    let color_layout = Self::create_color_layout(
      device,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Color layout",
    )
    .on_err(|_err| unsafe {
      main_layout.destroy_self(device);
      skybox_layout.destroy_self(device);
    })?;
    let text_layout = Self::create_text_layout(
      device,
      descriptor_pool,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text layout",
    )
    .on_err(|_err| unsafe {
      main_layout.destroy_self(device);
      skybox_layout.destroy_self(device);
      color_layout.destroy_self(device);
    })?;
    let layouts = [main_layout, skybox_layout, color_layout, text_layout];

    let main_shader = GraphicsShader::load_main(
      device,
      shader_loader,
      #[cfg(feature = "vl")]
      marker,
    )
    .map_err(PipelineCreationError::ShaderFailed)
    .on_err(|_err| unsafe {
      layouts.destroy_self(device);
    })?;
    let skybox_shader = GraphicsShader::load_skybox(
      device,
      shader_loader,
      #[cfg(feature = "vl")]
      marker,
    )
    .map_err(PipelineCreationError::ShaderFailed)
    .on_err(|_err| unsafe {
      layouts.destroy_self(device);
      main_shader.destroy_self(device);
    })?;
    let color_shader = GraphicsShader::load_color(
      device,
      shader_loader,
      #[cfg(feature = "vl")]
      marker,
    )
    .map_err(PipelineCreationError::ShaderFailed)
    .on_err(|_err| unsafe {
      layouts.destroy_self(device);
      main_shader.destroy_self(device);
      skybox_shader.destroy_self(device);
    })?;
    let text_shader = GraphicsShader::load_text(
      device,
      shader_loader,
      #[cfg(feature = "vl")]
      marker,
    )
    .map_err(PipelineCreationError::ShaderFailed)
    .on_err(|_err| unsafe {
      layouts.destroy_self(device);
      main_shader.destroy_self(device);
      skybox_shader.destroy_self(device);
      color_shader.destroy_self(device);
    })?;
    let shaders = [main_shader, skybox_shader, color_shader, text_shader];

    let handles = Self::create_with_base(
      device,
      Layouts {
        main: main_layout,
        skybox: skybox_layout,
        color: color_layout,
        text: text_layout,
      },
      Shaders {
        main: main_shader,
        skybox: skybox_shader,
        color: color_shader,
        text: text_shader,
      },
      cache,
      PipelineHandles {
        main: vk::Pipeline::null(),
        skybox: vk::Pipeline::null(),
        color: vk::Pipeline::null(),
        text: vk::Pipeline::null(),
      },
      render_format,
      extent,
      sample_count,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_err| unsafe {
      layouts.destroy_self(device);
      shaders.destroy_self(device);
    })?;

    let main = Pipeline {
      layout: main_layout,
      current: handles.main,
      shader: main_shader,
      old: None,
    };
    let skybox = Pipeline {
      layout: skybox_layout,
      current: handles.skybox,
      shader: skybox_shader,
      old: None,
    };
    let color = Pipeline {
      layout: color_layout,
      current: handles.color,
      shader: color_shader,
      old: None,
    };
    let text = Pipeline {
      layout: text_layout,
      current: handles.text,
      shader: text_shader,
      old: None,
    };

    Ok(Self {
      main,
      skybox,
      color,
      text,
    })
  }

  // create a new pipeline and mark the other as old
  pub fn recreate(
    &mut self,
    device: &ash::Device,
    cache: vk::PipelineCache,
    render_format: vk::Format,
    extent: vk::Extent2D,
    sample_count: usize,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<(), PipelineCreationError> {
    assert!(self.main.old.is_none());
    assert!(self.skybox.old.is_none());
    assert!(self.color.old.is_none());
    assert!(self.text.old.is_none());

    let mut new = Self::create_with_base(
      device,
      Layouts {
        main: self.main.layout,
        skybox: self.skybox.layout,
        color: self.color.layout,
        text: self.text.layout,
      },
      Shaders {
        main: self.main.shader,
        skybox: self.skybox.shader,
        color: self.color.shader,
        text: self.text.shader,
      },
      cache,
      PipelineHandles {
        main: self.main.current,
        skybox: self.skybox.current,
        color: self.color.current,
        text: self.text.current,
      },
      render_format,
      extent,
      sample_count,
      #[cfg(feature = "vl")]
      marker,
    )?;

    let old_main = {
      mem::swap(&mut self.main.current, &mut new.main);
      new.main
    };
    let old_skybox = {
      mem::swap(&mut self.skybox.current, &mut new.skybox);
      new.skybox
    };
    let old_color = {
      mem::swap(&mut self.color.current, &mut new.color);
      new.color
    };
    let old_text = {
      mem::swap(&mut self.text.current, &mut new.text);
      new.text
    };

    self.main.old = Some(old_main);
    self.skybox.old = Some(old_skybox);
    self.color.old = Some(old_color);
    self.text.old = Some(old_text);
    Ok(())
  }

  #[allow(dead_code)]
  pub fn revert_recreate(&mut self, device: &ash::Device) {
    unsafe {
      self.main.current.destroy_self(device);
      self.skybox.current.destroy_self(device);
      self.color.current.destroy_self(device);
      self.text.current.destroy_self(device);
    }
    let mut temp = None;
    mem::swap(&mut self.main.old, &mut temp);
    self.main.current = temp.unwrap();

    let mut temp = None;
    mem::swap(&mut self.skybox.old, &mut temp);
    self.skybox.current = temp.unwrap();

    let mut temp = None;
    mem::swap(&mut self.color.old, &mut temp);
    self.color.current = temp.unwrap();

    let mut temp = None;
    mem::swap(&mut self.text.old, &mut temp);
    self.text.current = temp.unwrap();
  }

  // destroy old pipeline once it stops being used
  pub unsafe fn destroy_old(&mut self, device: &ash::Device, cur_total_frame: usize) {
    if let Some(graphics_old) = self.main.old {
      log::debug!("[Frame {}] Destroying old pipelines", cur_total_frame);
      let skybox_old = self.skybox.old.unwrap();
      let color_old = self.color.old.unwrap();
      let text_old = self.text.old.unwrap();
      device.destroy_pipeline(graphics_old, None);
      device.destroy_pipeline(skybox_old, None);
      device.destroy_pipeline(color_old, None);
      device.destroy_pipeline(text_old, None);
      self.main.old = None;
      self.skybox.old = None;
      self.color.old = None;
      self.text.old = None;
    }
  }

  fn create_graphics_layout(
    device: &ash::Device,
    descriptor_pool: &DescriptorPool,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
    #[cfg(feature = "vl")] name: &CStr,
  ) -> Result<vk::PipelineLayout, OutOfMemoryError> {
    let push_constant_range = vk::PushConstantRange {
      stage_flags: vk::ShaderStageFlags::VERTEX,
      offset: 0,
      size: size_of::<GraphicsPushConstants>() as u32,
    };
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: 1,
      p_set_layouts: &descriptor_pool.textures_layout,
      push_constant_range_count: 1,
      p_push_constant_ranges: &push_constant_range,
      ..Default::default()
    };
    unsafe {
      let layout = device.create_pipeline_layout(&layout_create_info, None)?;
      #[cfg(feature = "vl")]
      marker.set_obj_name(vk::ObjectType::PIPELINE_LAYOUT, layout.as_raw(), name)?;
      Ok(layout)
    }
  }

  fn create_skybox_layout(
    device: &ash::Device,
    descriptor_pool: &DescriptorPool,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
    #[cfg(feature = "vl")] name: &CStr,
  ) -> Result<vk::PipelineLayout, OutOfMemoryError> {
    let push_constant_range = vk::PushConstantRange {
      stage_flags: vk::ShaderStageFlags::VERTEX,
      offset: 0,
      size: size_of::<Matrix4<f32>>() as u32,
    };
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      flags: vk::PipelineLayoutCreateFlags::empty(),
      set_layout_count: 1,
      p_set_layouts: &descriptor_pool.skybox_layout,
      push_constant_range_count: 1,
      p_push_constant_ranges: &push_constant_range,
      ..Default::default()
    };
    unsafe {
      let layout = device.create_pipeline_layout(&layout_create_info, None)?;
      #[cfg(feature = "vl")]
      marker.set_obj_name(vk::ObjectType::PIPELINE_LAYOUT, layout.as_raw(), name)?;
      Ok(layout)
    }
  }

  fn create_color_layout(
    device: &ash::Device,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
    #[cfg(feature = "vl")] name: &CStr,
  ) -> Result<vk::PipelineLayout, OutOfMemoryError> {
    let push_constant_range = vk::PushConstantRange {
      stage_flags: vk::ShaderStageFlags::VERTEX,
      offset: 0,
      size: size_of::<ColorPushConstants>() as u32,
    };
    let layout_create_info = vk::PipelineLayoutCreateInfo {
      flags: vk::PipelineLayoutCreateFlags::empty(),
      push_constant_range_count: 1,
      p_push_constant_ranges: &push_constant_range,
      ..Default::default()
    };
    unsafe {
      let layout = device.create_pipeline_layout(&layout_create_info, None)?;
      #[cfg(feature = "vl")]
      marker.set_obj_name(vk::ObjectType::PIPELINE_LAYOUT, layout.as_raw(), name)?;
      Ok(layout)
    }
  }

  fn create_text_layout(
    device: &ash::Device,
    descriptor_pool: &DescriptorPool,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
    #[cfg(feature = "vl")] name: &CStr,
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
    unsafe {
      let layout = device.create_pipeline_layout(&layout_create_info, None)?;
      #[cfg(feature = "vl")]
      marker.set_obj_name(vk::ObjectType::PIPELINE_LAYOUT, layout.as_raw(), name)?;
      Ok(layout)
    }
  }

  fn create_with_base(
    device: &ash::Device,
    layouts: Layouts,
    shaders: Shaders,
    cache: vk::PipelineCache,
    base_handles: PipelineHandles,
    render_format: vk::Format,
    extent: vk::Extent2D,
    sample_count: usize,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<PipelineHandles, PipelineCreationError> {
    let main_shader_stages = shaders.main.get_pipeline_shader_creation_info();
    let skybox_shader_stages = shaders.skybox.get_pipeline_shader_creation_info();
    let color_shader_stages = shaders.color.get_pipeline_shader_creation_info();
    let text_shader_stages = shaders.text.get_pipeline_shader_creation_info();

    let main_vertex_input_state = vertex_input_state_create_info!(TexturedVertex);
    let main_vertex_input_state = main_vertex_input_state.get();
    let skybox_vertex_input_state = main_vertex_input_state;
    let color_vertex_input_state = main_vertex_input_state;
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

    let main_rasterization_state_ci = vk::PipelineRasterizationStateCreateInfo {
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
    let skybox_rasterization_state_ci = vk::PipelineRasterizationStateCreateInfo {
      cull_mode: vk::CullModeFlags::BACK,
      front_face: vk::FrontFace::COUNTER_CLOCKWISE,
      ..main_rasterization_state_ci
    };
    let text_rasterization_state_ci = vk::PipelineRasterizationStateCreateInfo {
      cull_mode: vk::CullModeFlags::BACK,
      front_face: vk::FrontFace::COUNTER_CLOCKWISE,
      ..main_rasterization_state_ci
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
      depth_compare_op: vk::CompareOp::LESS_OR_EQUAL,
      depth_bounds_test_enable: vk::FALSE,
      min_depth_bounds: 0.0,
      max_depth_bounds: 1.0,
      stencil_test_enable: vk::FALSE,
      ..Default::default()
    };
    let color_stencil_state_ci = vk::PipelineDepthStencilStateCreateInfo {
      depth_test_enable: vk::FALSE,
      ..depth_stencil_state_ci
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

    let mut main_flags = vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
    let mut skybox_flags = vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
    let mut color_flags = vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
    let mut text_flags = vk::PipelineCreateFlags::ALLOW_DERIVATIVES;
    if !base_handles.main.is_null() {
      main_flags = main_flags.bitor(vk::PipelineCreateFlags::DERIVATIVE);
    }
    if !base_handles.skybox.is_null() {
      skybox_flags = skybox_flags.bitor(vk::PipelineCreateFlags::DERIVATIVE);
    }
    if !base_handles.color.is_null() {
      color_flags = color_flags.bitor(vk::PipelineCreateFlags::DERIVATIVE);
    }
    if !base_handles.text.is_null() {
      text_flags = text_flags.bitor(vk::PipelineCreateFlags::DERIVATIVE);
    }
    let main_create_info = vk::GraphicsPipelineCreateInfo {
      p_next: addr_of!(rendering_create_info) as *const c_void,
      flags: main_flags,
      stage_count: main_shader_stages.len() as u32,
      p_stages: main_shader_stages.as_ptr(),
      p_vertex_input_state: main_vertex_input_state,
      p_input_assembly_state: &input_assembly_state,
      p_tessellation_state: ptr::null(),
      p_viewport_state: &viewport_state,
      p_rasterization_state: &main_rasterization_state_ci,
      p_multisample_state: &multisample_state_ci,
      p_depth_stencil_state: &depth_stencil_state_ci,
      p_color_blend_state: &color_blend_state,
      p_dynamic_state: &dynamic_state_ci,
      layout: layouts.main,
      render_pass: vk::RenderPass::null(), // replaced by dynamic rendering
      subpass: 0,
      base_pipeline_handle: base_handles.main,
      base_pipeline_index: -1, // -1 for null
      ..Default::default()
    };
    let skybox_create_info = vk::GraphicsPipelineCreateInfo {
      flags: skybox_flags,
      stage_count: skybox_shader_stages.len() as u32,
      p_stages: skybox_shader_stages.as_ptr(),
      p_vertex_input_state: skybox_vertex_input_state,
      p_rasterization_state: &skybox_rasterization_state_ci,
      p_dynamic_state: ptr::null(),
      layout: layouts.skybox,
      base_pipeline_handle: base_handles.skybox,
      base_pipeline_index: -1, // -1 for null
      ..main_create_info
    };
    let color_create_info = vk::GraphicsPipelineCreateInfo {
      flags: color_flags,
      stage_count: color_shader_stages.len() as u32,
      p_stages: color_shader_stages.as_ptr(),
      p_vertex_input_state: color_vertex_input_state,
      p_depth_stencil_state: &color_stencil_state_ci,
      p_dynamic_state: ptr::null(),
      layout: layouts.color,
      base_pipeline_handle: base_handles.color,
      base_pipeline_index: -1, // -1 for null
      ..main_create_info
    };
    let text_create_info = vk::GraphicsPipelineCreateInfo {
      flags: text_flags,
      stage_count: text_shader_stages.len() as u32,
      p_stages: text_shader_stages.as_ptr(),
      p_vertex_input_state: text_vertex_input_state,
      p_rasterization_state: &text_rasterization_state_ci,
      layout: layouts.text,
      base_pipeline_handle: base_handles.text,
      base_pipeline_index: -1, // -1 for null
      ..main_create_info
    };
    let create_infos = [
      main_create_info,
      skybox_create_info,
      color_create_info,
      text_create_info,
    ];

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
    let main = pipelines[0];
    let skybox = pipelines[1];
    let color = pipelines[2];
    let text = pipelines[3];

    #[cfg(feature = "vl")]
    unsafe {
      marker.set_obj_name(
        vk::ObjectType::PIPELINE,
        main.as_raw(),
        c"Main Graphics Pipeline",
      )?;
      marker.set_obj_name(
        vk::ObjectType::PIPELINE,
        skybox.as_raw(),
        c"Skybox Pipeline",
      )?;
      marker.set_obj_name(vk::ObjectType::PIPELINE, color.as_raw(), c"Color Pipeline")?;
      marker.set_obj_name(vk::ObjectType::PIPELINE, text.as_raw(), c"Text Pipeline")?;
    }

    Ok(PipelineHandles {
      main,
      skybox,
      color,
      text,
    })
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
    self.main.destroy_self(device);
    self.skybox.destroy_self(device);
    self.color.destroy_self(device);
    self.text.destroy_self(device);
  }
}

impl DeviceManuallyDestroyed for Pipeline {
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
