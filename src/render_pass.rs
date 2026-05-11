use std::{marker::PhantomData, ptr};

use ash::vk;
use vkobjects::errors::OutOfMemoryError;

use crate::IMAGE_FORMAT;

pub fn create_render_pass(device: &ash::Device) -> Result<vk::RenderPass, OutOfMemoryError> {
  let image_attachment = vk::AttachmentDescription {
    flags: vk::AttachmentDescriptionFlags::empty(),
    format: IMAGE_FORMAT,
    samples: vk::SampleCountFlags::TYPE_1,
    load_op: vk::AttachmentLoadOp::CLEAR,
    store_op: vk::AttachmentStoreOp::STORE,
    stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
    stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
    initial_layout: vk::ImageLayout::UNDEFINED,
    final_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL, // layout after render pass finishes
  };

  let attachment_ref = vk::AttachmentReference {
    attachment: 0,
    layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
  };

  let image_subpass = vk::SubpassDescription {
    flags: vk::SubpassDescriptionFlags::empty(),
    pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
    input_attachment_count: 0,
    p_input_attachments: ptr::null(),
    // output attachments
    color_attachment_count: 1,
    p_color_attachments: &attachment_ref,
    p_resolve_attachments: ptr::null(),
    p_depth_stencil_attachment: ptr::null(),
    preserve_attachment_count: 0,
    p_preserve_attachments: ptr::null(),
    _marker: PhantomData,
  };

  let dependencies = [
    // change access flags to attachment before subpass begins
    vk::SubpassDependency {
      src_subpass: vk::SUBPASS_EXTERNAL,
      dst_subpass: 0, // image_subpass
      src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
      dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
      src_access_mask: vk::AccessFlags::NONE,
      dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
      dependency_flags: vk::DependencyFlags::empty(),
    },
    // wait for subpass to finish before doing any transfer
    vk::SubpassDependency {
      src_subpass: 0,
      dst_subpass: vk::SUBPASS_EXTERNAL,
      src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
      dst_stage_mask: vk::PipelineStageFlags::TRANSFER,
      src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
      dst_access_mask: vk::AccessFlags::NONE,
      dependency_flags: vk::DependencyFlags::empty(),
    },
  ];

  let create_info = vk::RenderPassCreateInfo {
    s_type: vk::StructureType::RENDER_PASS_CREATE_INFO,
    flags: vk::RenderPassCreateFlags::empty(),
    p_next: ptr::null(),
    attachment_count: 1,
    p_attachments: &image_attachment,
    subpass_count: 1,
    p_subpasses: &image_subpass,
    dependency_count: dependencies.len() as u32,
    p_dependencies: dependencies.as_ptr(),
    _marker: PhantomData,
  };
  unsafe {
    device
      .create_render_pass(&create_info, None)
      .map_err(|err| err.into())
  }
}

pub fn create_framebuffer(
  device: &ash::Device,
  render_pass: vk::RenderPass,
  image_view: vk::ImageView,
  extent: vk::Extent2D,
) -> Result<vk::Framebuffer, OutOfMemoryError> {
  let create_info = vk::FramebufferCreateInfo {
    s_type: vk::StructureType::FRAMEBUFFER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::FramebufferCreateFlags::empty(),
    render_pass,
    attachment_count: 1,
    p_attachments: &image_view,
    width: extent.width,
    height: extent.height,
    layers: 1,
    _marker: PhantomData,
  };
  unsafe {
    device
      .create_framebuffer(&create_info, None)
      .map_err(|err| err.into())
  }
}
