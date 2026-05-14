use std::{marker::PhantomData, ptr};

use ash::vk;
use vkobjects::errors::OutOfMemoryError;

pub fn create_render_pass(
  device: &ash::Device,
  format: vk::Format,
) -> Result<vk::RenderPass, OutOfMemoryError> {
  let image_attachment = [vk::AttachmentDescription {
    flags: vk::AttachmentDescriptionFlags::empty(),
    format,
    samples: vk::SampleCountFlags::TYPE_1,
    load_op: vk::AttachmentLoadOp::CLEAR,
    store_op: vk::AttachmentStoreOp::STORE,
    stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
    stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
    initial_layout: vk::ImageLayout::UNDEFINED,
    final_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL, // layout after render pass finishes
  }];

  let attachment_ref = [vk::AttachmentReference {
    attachment: 0,
    layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
  }];

  let image_subpass = [vk::SubpassDescription::default()
    .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
    .color_attachments(&attachment_ref)];

  let dependencies = [
    // finish subpass before doing the blit (or copy) operation
    vk::SubpassDependency {
      src_subpass: 0,
      dst_subpass: vk::SUBPASS_EXTERNAL,
      src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
      dst_stage_mask: vk::PipelineStageFlags::TRANSFER, // blit / copy
      src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
      dst_access_mask: vk::AccessFlags::TRANSFER_READ,
      dependency_flags: vk::DependencyFlags::empty(),
    },
  ];

  let create_info = vk::RenderPassCreateInfo::default()
    .attachments(&image_attachment)
    .subpasses(&image_subpass)
    .dependencies(&dependencies);
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
