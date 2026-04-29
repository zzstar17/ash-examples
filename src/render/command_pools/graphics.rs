use std::{cmp::Ordering, marker::PhantomData, ptr};

use ash::vk;

use crate::{
  render::{
    descriptor_sets::DescriptorPool,
    device_destroyable::DeviceManuallyDestroyed,
    errors::OutOfMemoryError,
    gpu_data::GPUData,
    initialization::device::QueueFamilies,
    pipelines::GraphicsPipeline,
    render_object::{RenderPosition, QUAD_INDICES},
    render_targets::RenderTargets,
    RENDER_EXTENT,
  },
  utility, BACKGROUND_COLOR, OUT_OF_BOUNDS_AREA_COLOR,
};

use super::dependency_info;

pub struct GraphicsCommandBufferPool {
  pool: vk::CommandPool,
  pub main: vk::CommandBuffer,
}

impl GraphicsCommandBufferPool {
  pub fn create(
    device: &ash::Device,
    queue_families: &QueueFamilies,
    #[cfg(feature = "vl")] marker: &crate::render::initialization::DebugUtilsMarker,
  ) -> Result<Self, vk::Result> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(
      device,
      flags,
      queue_families.graphics.index,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"graphics command pool",
    )?;

    #[cfg(feature = "vl")]
    let command_buffer_names = [c"primary"];
    let main = super::allocate_primary_command_buffers(
      device,
      pool,
      1,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      &command_buffer_names,
    )?[0];

    Ok(Self { pool, main })
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), OutOfMemoryError> {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .map_err(|err| err.into())
  }

  pub unsafe fn record_main(
    &mut self,
    frame_i: usize,
    device: &ash::Device,

    render_pass: vk::RenderPass,
    render_targets: &RenderTargets,

    swapchain_image: vk::Image,
    swapchain_extent: vk::Extent2D,

    pipeline: &GraphicsPipeline,

    descriptor_pool: &DescriptorPool,
    data: &GPUData,
    position: &RenderPosition, // Ferris's position

    screenshot_buffer: Option<vk::Buffer>,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.main;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    let render_width = RENDER_EXTENT.width as i32;
    let render_height = RENDER_EXTENT.height as i32;
    let swapchain_width = swapchain_extent.width as i32;
    let swapchain_height = swapchain_extent.height as i32;

    // do a copy operation instead of blit if true
    let just_copying = (render_width == swapchain_width && swapchain_height >= render_height)
      || (render_height == swapchain_height && swapchain_width >= render_width);

    // in this case the render pass takes care of all internal queue synchronization
    {
      let clear_value = vk::ClearValue {
        color: BACKGROUND_COLOR,
      };
      let render_pass_begin_info = vk::RenderPassBeginInfo {
        s_type: vk::StructureType::RENDER_PASS_BEGIN_INFO,
        p_next: ptr::null(),
        render_pass,
        framebuffer: render_targets.framebuffers[frame_i],
        // whole image
        render_area: vk::Rect2D {
          offset: vk::Offset2D { x: 0, y: 0 },
          extent: RENDER_EXTENT,
        },
        clear_value_count: 1,
        p_clear_values: &clear_value,
        _marker: PhantomData,
      };
      device.cmd_begin_render_pass(cb, &render_pass_begin_info, vk::SubpassContents::INLINE);

      device.cmd_bind_descriptor_sets(
        cb,
        vk::PipelineBindPoint::GRAPHICS,
        pipeline.layout,
        0,
        &[descriptor_pool.texture_set],
        &[],
      );
      device.cmd_push_constants(
        cb,
        pipeline.layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        utility::any_as_u8_slice(position),
      );
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline.current);
      device.cmd_bind_vertex_buffers(cb, 0, &[data.vertex_buffer], &[0]);
      device.cmd_bind_index_buffer(cb, data.index_buffer, 0, vk::IndexType::UINT16);
      device.cmd_draw_indexed(cb, QUAD_INDICES.len() as u32, 1, 0, 0, 0);

      device.cmd_end_render_pass(cb);
    }

    // 1 mip_level / 1 array layer
    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    // prepare and clear swapchain image
    {
      let swapchain_transfer_dst_layout = vk::ImageMemoryBarrier2 {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::NONE,
        dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        src_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT, // image_available semaphore
        dst_stage_mask: vk::PipelineStageFlags2::CLEAR,
        old_layout: vk::ImageLayout::UNDEFINED,
        new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image: swapchain_image,
        subresource_range,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(
        cb,
        &dependency_info(&[], &[], &[swapchain_transfer_dst_layout]),
      );

      device.cmd_clear_color_image(
        cb,
        swapchain_image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        &OUT_OF_BOUNDS_AREA_COLOR,
        &[subresource_range],
      );

      let flush_clear = vk::MemoryBarrier2 {
        s_type: vk::StructureType::MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        src_stage_mask: vk::PipelineStageFlags2::CLEAR,
        dst_stage_mask: if just_copying {
          vk::PipelineStageFlags2::COPY
        } else {
          vk::PipelineStageFlags2::BLIT
        },
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[flush_clear], &[], &[]));
    }

    let layers = vk::ImageSubresourceLayers {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      mip_level: 0,
      base_array_layer: 0,
      layer_count: 1,
    };

    // screenshot
    if let Some(buffer) = screenshot_buffer {
      // full image
      let region = vk::BufferImageCopy {
        image_subresource: layers,
        image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
        image_extent: vk::Extent3D {
          width: RENDER_EXTENT.width,
          height: RENDER_EXTENT.height,
          depth: 1,
        },
        buffer_offset: 0,
        buffer_image_height: 0, // densely packed
        buffer_row_length: 0,
      };
      device.cmd_copy_image_to_buffer(
        cb,
        render_targets.images[frame_i],
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        buffer,
        &[region],
      );

      let flush_to_host = vk::BufferMemoryBarrier2 {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::HOST_READ,
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::HOST,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer,
        offset: 0,
        size: vk::WHOLE_SIZE,
        _marker: PhantomData,
      };
      // make sure memory contents are flushed for the next screenshot request
      let flush_to_next_copy_write = vk::BufferMemoryBarrier2 {
        s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::COPY,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        buffer,
        offset: 0,
        size: vk::WHOLE_SIZE,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(
        cb,
        &dependency_info(&[], &[flush_to_host, flush_to_next_copy_write], &[]),
      );
    }

    if just_copying {
      let x_offset = (render_width - swapchain_width).abs() / 2;
      let y_offset = (render_height - swapchain_height).abs() / 2;
      let region = vk::ImageCopy {
        src_subresource: layers,
        src_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
        dst_subresource: layers,
        dst_offset: vk::Offset3D {
          x: x_offset,
          y: y_offset,
          z: 0,
        },
        extent: vk::Extent3D {
          width: RENDER_EXTENT.width,
          height: RENDER_EXTENT.height,
          depth: 1,
        },
      };
      device.cmd_copy_image(
        cb,
        render_targets.images[frame_i],
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        swapchain_image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        &[region],
      )
    } else {
      let blit_region = get_centered_blit_region(
        render_width,
        render_height,
        swapchain_width,
        swapchain_height,
        layers,
        layers,
      );
      device.cmd_blit_image(
        cb,
        render_targets.images[frame_i],
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        swapchain_image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        &[blit_region],
        vk::Filter::NEAREST,
      );
    }

    {
      let swapchain_presentation_layout = vk::ImageMemoryBarrier2 {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::NONE,
        src_stage_mask: if just_copying {
          vk::PipelineStageFlags2::COPY
        } else {
          vk::PipelineStageFlags2::BLIT
        },
        dst_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        new_layout: vk::ImageLayout::PRESENT_SRC_KHR,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image: swapchain_image,
        subresource_range,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(
        cb,
        &dependency_info(&[], &[], &[swapchain_presentation_layout]),
      );
    }

    device.end_command_buffer(cb)?;
    Ok(())
  }
}

impl DeviceManuallyDestroyed for GraphicsCommandBufferPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}

fn get_centered_blit_region(
  src_width: i32,
  src_height: i32,
  dst_width: i32,
  dst_height: i32,
  src_subresource: vk::ImageSubresourceLayers,
  dst_subresource: vk::ImageSubresourceLayers,
) -> vk::ImageBlit {
  let width_diff = dst_width - src_width;
  let height_diff = dst_height - src_height;
  let dst_start;
  let dst_end;
  match width_diff.cmp(&height_diff) {
    Ordering::Greater => {
      // clamp to height
      let ratio = dst_height as f32 / src_height as f32;
      let resized_width = (src_width as f32 * ratio) as i32;

      let half = (dst_width - resized_width) / 2;
      dst_start = [half, 0];
      dst_end = [half + resized_width, dst_height];
    }
    Ordering::Equal => {
      dst_start = [0, 0];
      dst_end = [dst_width, dst_height];
    }
    Ordering::Less => {
      // clamp to width
      let ratio = dst_width as f32 / src_width as f32;
      let resized_height = (src_height as f32 * ratio) as i32;

      let half = (dst_height - resized_height) / 2;
      dst_start = [0, half];
      dst_end = [dst_width, half + resized_height];
    }
  }
  vk::ImageBlit {
    src_subresource,
    src_offsets: [
      vk::Offset3D { x: 0, y: 0, z: 0 },
      vk::Offset3D {
        x: src_width,
        y: src_height,
        z: 1,
      },
    ],
    dst_subresource,
    dst_offsets: [
      vk::Offset3D {
        x: dst_start[0],
        y: dst_start[1],
        z: 0,
      },
      vk::Offset3D {
        x: dst_end[0],
        y: dst_end[1],
        z: 1,
      },
    ],
  }
}
