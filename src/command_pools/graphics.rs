use std::{marker::PhantomData, ptr};

use ash::vk;
use vkinitialization::device::QueueFamilies;
use vkobjects::{errors::OutOfMemoryError, DeviceManuallyDestroyed};

use crate::{
  gpu_data::GPUData, pipelines::GraphicsPipeline, BACKGROUND_COLOR, IMAGE_HEIGHT, IMAGE_WIDTH,
  INDICES,
};

use super::dependency_info;

pub struct GraphicsCommandBufferPool {
  pool: vk::CommandPool,
  pub triangle: vk::CommandBuffer,
}

impl GraphicsCommandBufferPool {
  pub fn create(
    device: &ash::Device,
    queue_families: &QueueFamilies,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, OutOfMemoryError> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(
      device,
      flags,
      queue_families.graphics.index,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"graphics",
    )?;

    let triangle = super::allocate_primary_command_buffers(
      device,
      pool,
      1,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      &[c"graphics"],
    )?[0];

    Ok(Self { pool, triangle })
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), vk::Result> {
    device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
  }

  pub unsafe fn record_triangle(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    render_pass: vk::RenderPass,
    pipeline: &GraphicsPipeline,
    data: &GPUData,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.triangle;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    {
      let clear_value = vk::ClearValue {
        color: BACKGROUND_COLOR,
      };
      let render_pass_begin_info = vk::RenderPassBeginInfo {
        s_type: vk::StructureType::RENDER_PASS_BEGIN_INFO,
        p_next: ptr::null(),
        render_pass,
        framebuffer: data.r_target_framebuffer,
        // whole image
        render_area: vk::Rect2D {
          offset: vk::Offset2D { x: 0, y: 0 },
          extent: vk::Extent2D {
            width: IMAGE_WIDTH,
            height: IMAGE_HEIGHT,
          },
        },
        clear_value_count: 1,
        p_clear_values: &clear_value,
        _marker: PhantomData,
      };
      device.cmd_begin_render_pass(cb, &render_pass_begin_info, vk::SubpassContents::INLINE);

      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline.pipeline);
      device.cmd_bind_vertex_buffers(cb, 0, &[data.vertex_buffer], &[0]);
      device.cmd_bind_index_buffer(cb, data.index_buffer, 0, vk::IndexType::UINT16);
      device.cmd_draw_indexed(cb, INDICES.len() as u32, 1, 0, 0, 0);

      device.cmd_end_render_pass(cb);
    }

    // image has 1 mip_level / 1 array layer
    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    // After the render pass finishes the image will already have the correct layout, so only a
    // queue ownership transfer is necessary
    let graphics_family = queue_families.graphics.index;
    let transfer_family = queue_families
      .transfer
      .unwrap_or(queue_families.graphics)
      .index;
    if graphics_family != transfer_family {
      let release = vk::ImageMemoryBarrier2 {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_stage_mask: vk::PipelineStageFlags2::TRANSFER, // from render pass
        dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
        src_access_mask: vk::AccessFlags2::NONE, // from render pass
        dst_access_mask: vk::AccessFlags2::NONE, // NONE for ownership release
        old_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        src_queue_family_index: graphics_family,
        dst_queue_family_index: transfer_family,
        image: data.render_target,
        subresource_range,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[release]));
    }

    device.end_command_buffer(self.triangle)?;

    Ok(())
  }
}

impl DeviceManuallyDestroyed for GraphicsCommandBufferPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
