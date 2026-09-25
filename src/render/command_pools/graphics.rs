use std::{cmp::Ordering, marker::PhantomData, ops::BitOr, ptr};

use ash::vk::{self, ClearDepthStencilValue};
use ash_slug::SlugPushConstants;
use cgmath::{Matrix4, Vector4};
use vkinitialization::device::QueueFamilies;
use vkobjects::{errors::OutOfMemoryError, utility, DeviceManuallyDestroyed};

use crate::{
  render::{
    command_pools::{
      ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_LAYERS, ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_RANGE,
      ONE_LAYER_DEPTH_IMAGE_SUBRESOURCE_RANGE,
    },
    descriptor_sets::DescriptorPool,
    gpu_data::GPUData,
    pipelines::{GraphicsPipeline, GraphicsPushConstants, TextPipeline},
    render_targets::RenderTargets,
    RENDER_EXTENT, RENDER_SIZE,
  },
  scene::DrawMatrices,
  BACKGROUND_COLOR, OUT_OF_BOUNDS_AREA_COLOR,
};

use super::dependency_info;

#[derive(Debug, Clone, Copy)]
pub struct GraphicsCommandBufferPool {
  pool: vk::CommandPool,
  pub main: vk::CommandBuffer,
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
      c"graphics command pool",
    )?;

    #[cfg(feature = "vl")]
    let command_buffer_names = [c"Main"];
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

  pub unsafe fn reset(&self, device: &ash::Device) -> Result<(), OutOfMemoryError> {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .map_err(|err| err.into())
  }

  pub unsafe fn begin_recording(&self, device: &ash::Device) -> Result<(), OutOfMemoryError> {
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(self.main, &begin_info)?;
    Ok(())
  }

  pub unsafe fn record_copy_staging_buffer_to_image(
    &self,
    device: &ash::Device,
    staging: vk::Buffer,
    staging_offset: u64,
    dst: vk::Image,
    image_extent: vk::Extent2D,
    final_layout: vk::ImageLayout,
    img_src_stage_mask: vk::PipelineStageFlags2,
    img_src_access_mask: vk::AccessFlags2,
    img_dst_stage_mask: vk::PipelineStageFlags2,
    img_dst_access_mask: vk::AccessFlags2,
  ) {
    let transfer_dst_layout = vk::ImageMemoryBarrier2 {
      src_stage_mask: img_src_stage_mask,
      dst_stage_mask: vk::PipelineStageFlags2::COPY,
      src_access_mask: img_src_access_mask,
      dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
      old_layout: vk::ImageLayout::UNDEFINED,
      new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image: dst,
      subresource_range: ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_RANGE,
      ..Default::default()
    };
    device.cmd_pipeline_barrier2(
      self.main,
      &dependency_info(&[], &[], &[transfer_dst_layout]),
    );

    let copy_region = vk::BufferImageCopy {
      buffer_offset: staging_offset,
      buffer_row_length: 0,   // 0 because buffer is tightly packed
      buffer_image_height: 0, // 0 because buffer is tightly packed
      image_subresource: ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_LAYERS,
      image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
      image_extent: vk::Extent3D {
        width: image_extent.width,
        height: image_extent.height,
        depth: 1,
      },
    };
    device.cmd_copy_buffer_to_image(
      self.main,
      staging,
      dst,
      vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      &[copy_region],
    );

    let change_to_final_layout = vk::ImageMemoryBarrier2 {
      src_stage_mask: vk::PipelineStageFlags2::COPY,
      dst_stage_mask: img_dst_stage_mask,
      src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
      dst_access_mask: img_dst_access_mask,
      old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      new_layout: final_layout,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image: dst,
      subresource_range: ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_RANGE,
      ..Default::default()
    };
    device.cmd_pipeline_barrier2(
      self.main,
      &dependency_info(&[], &[], &[change_to_final_layout]),
    );
  }

  pub unsafe fn record_update_text_ui(
    &self,
    device: &ash::Device,
    descriptor_pool: &DescriptorPool,
    data: &GPUData,
    text_pipeline: &TextPipeline,
  ) {
    let cb = self.main;

    let wait_ui = vk::ImageMemoryBarrier2 {
      src_access_mask: vk::AccessFlags2::NONE,
      dst_access_mask: vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
      src_stage_mask: vk::PipelineStageFlags2::FRAGMENT_SHADER, // previous main shader operation
      dst_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
      old_layout: vk::ImageLayout::UNDEFINED, // we don't care about old contents
      new_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
      image: data.text_ui,
      subresource_range: ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_RANGE,
      ..Default::default()
    };
    device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[wait_ui]));

    let clear_value = vk::ClearValue {
      color: vk::ClearColorValue {
        float32: [0.0, 0.0, 0.0, 0.1],
      },
    };
    let color_attachments = [vk::RenderingAttachmentInfo {
      image_view: data.text_ui_view,
      image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
      resolve_mode: vk::ResolveModeFlags::NONE,
      resolve_image_view: vk::ImageView::null(),
      resolve_image_layout: vk::ImageLayout::UNDEFINED,
      load_op: vk::AttachmentLoadOp::CLEAR,
      store_op: vk::AttachmentStoreOp::STORE,
      clear_value,
      ..Default::default()
    }];
    let rendering_info = vk::RenderingInfo {
      flags: vk::RenderingFlags::empty(),
      render_area: vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent: data.text_ui_size,
      },
      layer_count: 1,
      view_mask: 0,
      color_attachment_count: color_attachments.len() as u32,
      p_color_attachments: color_attachments.as_ptr(),
      p_depth_attachment: ptr::null(),
      p_stencil_attachment: ptr::null(),
      ..Default::default()
    };
    device.cmd_begin_rendering(cb, &rendering_info);

    let text_pc =
      SlugPushConstants::new_2d(RENDER_SIZE.x, RENDER_SIZE.y, [0.0, data.text_ui_line_size]);

    device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, text_pipeline.current);
    device.cmd_bind_descriptor_sets(
      cb,
      vk::PipelineBindPoint::GRAPHICS,
      text_pipeline.layout,
      0,
      &[descriptor_pool.text_set],
      &[],
    );
    device.cmd_push_constants(
      cb,
      text_pipeline.layout,
      vk::ShaderStageFlags::VERTEX,
      0,
      utility::any_as_u8_slice(&text_pc),
    );
    device.cmd_bind_vertex_buffers(cb, 0, &[data.text.buffers.device.vertices], &[0]);
    device.cmd_bind_index_buffer(
      cb,
      data.text.buffers.device.indices,
      0,
      vk::IndexType::UINT32,
    );
    device.cmd_draw_indexed(cb, data.text.device_index_count, 1, 0, 0, 0);

    device.cmd_end_rendering(cb);

    let wait_ui = vk::ImageMemoryBarrier2 {
      src_access_mask: vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
      dst_access_mask: vk::AccessFlags2::SHADER_SAMPLED_READ,
      src_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
      dst_stage_mask: vk::PipelineStageFlags2::FRAGMENT_SHADER,
      old_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
      new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
      image: data.text_ui,
      subresource_range: ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_RANGE,
      ..Default::default()
    };
    device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[wait_ui]));
  }

  pub unsafe fn record_main(
    &self,
    frame_i: usize,
    device: &ash::Device,
    render_targets: &RenderTargets,

    swapchain_image: vk::Image,
    swapchain_extent: vk::Extent2D,

    pipeline: &GraphicsPipeline,
    text_pipeline: &TextPipeline,

    descriptor_pool: &DescriptorPool,
    data: &GPUData,
    matrices: &DrawMatrices,

    screenshot_buffer: Option<vk::Buffer>,
    draw_text: bool,
  ) {
    let cb = self.main;

    let render_width = RENDER_EXTENT.width as i32;
    let render_height = RENDER_EXTENT.height as i32;
    let swapchain_width = swapchain_extent.width as i32;
    let swapchain_height = swapchain_extent.height as i32;

    // do a copy operation instead of blit if true
    let just_copying = (render_width == swapchain_width && swapchain_height >= render_height)
      || (render_height == swapchain_height && swapchain_width >= render_width);

    let layers = vk::ImageSubresourceLayers {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      mip_level: 0,
      base_array_layer: 0,
      layer_count: 1,
    };

    // wait previous copy on render target
    {
      let wait_render_target_color = vk::ImageMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::NONE,
        dst_access_mask: vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
        src_stage_mask: vk::PipelineStageFlags2::COPY.bitor(vk::PipelineStageFlags2::BLIT), // previous copy operations
        dst_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        old_layout: vk::ImageLayout::UNDEFINED, // we don't care about old contents
        new_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        image: render_targets.color_images[frame_i],
        subresource_range: ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_RANGE,
        ..Default::default()
      };
      let wait_render_target_depth = vk::ImageMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::NONE,
        dst_access_mask: vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE
          .bitor(vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_READ),
        src_stage_mask: vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS
          .bitor(vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS), // previous operations
        dst_stage_mask: vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS
          .bitor(vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS),
        old_layout: vk::ImageLayout::UNDEFINED, // we don't care about old contents
        new_layout: vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
        image: render_targets.depth_images[frame_i],
        subresource_range: ONE_LAYER_DEPTH_IMAGE_SUBRESOURCE_RANGE,
        ..Default::default()
      };
      device.cmd_pipeline_barrier2(
        cb,
        &dependency_info(
          &[],
          &[],
          &[wait_render_target_color, wait_render_target_depth],
        ),
      );

      let clear_value = vk::ClearValue {
        color: BACKGROUND_COLOR,
      };
      let depth_clear_value = vk::ClearValue {
        depth_stencil: ClearDepthStencilValue {
          depth: 1.0,
          stencil: 0,
        },
      };
      let color_attachments = [vk::RenderingAttachmentInfo {
        image_view: render_targets.color_views[frame_i],
        image_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        resolve_mode: vk::ResolveModeFlags::NONE,
        resolve_image_view: vk::ImageView::null(),
        resolve_image_layout: vk::ImageLayout::UNDEFINED,
        load_op: vk::AttachmentLoadOp::CLEAR,
        store_op: vk::AttachmentStoreOp::STORE,
        clear_value,
        ..Default::default()
      }];
      let depth_attachment = vk::RenderingAttachmentInfo {
        image_view: render_targets.depth_views[frame_i],
        image_layout: vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
        resolve_mode: vk::ResolveModeFlags::NONE,
        resolve_image_view: vk::ImageView::null(),
        resolve_image_layout: vk::ImageLayout::UNDEFINED,
        load_op: vk::AttachmentLoadOp::CLEAR,
        store_op: vk::AttachmentStoreOp::STORE,
        clear_value: depth_clear_value,
        ..Default::default()
      };
      let rendering_info = vk::RenderingInfo {
        flags: vk::RenderingFlags::empty(),
        render_area: vk::Rect2D {
          offset: vk::Offset2D { x: 0, y: 0 },
          extent: RENDER_EXTENT,
        },
        layer_count: 1,
        view_mask: 0,
        color_attachment_count: color_attachments.len() as u32,
        p_color_attachments: color_attachments.as_ptr(),
        p_depth_attachment: &depth_attachment,
        p_stencil_attachment: ptr::null(),
        ..Default::default()
      };
      device.cmd_begin_rendering(cb, &rendering_info);

      // draw ferris
      device.cmd_bind_descriptor_sets(
        cb,
        vk::PipelineBindPoint::GRAPHICS,
        pipeline.layout,
        0,
        &[descriptor_pool.sprites_set],
        &[],
      );

      let graphics_push_constants = GraphicsPushConstants {
        matrix: matrices.ferris,
        tex_offset: data.sprite_buffers.texture_offsets.ferris.offset,
        tex_size: data.sprite_buffers.texture_offsets.ferris.size,
      };
      device.cmd_push_constants(
        cb,
        pipeline.layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        utility::any_as_u8_slice(&graphics_push_constants),
      );

      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, pipeline.current);
      device.cmd_bind_vertex_buffers(cb, 0, &[data.sprite_buffers.vertices], &[0]);
      device.cmd_bind_index_buffer(cb, data.sprite_buffers.indices, 0, vk::IndexType::UINT32);
      device.cmd_draw_indexed(
        cb,
        data.sprite_buffers.models.quad.indices_len as u32,
        1,
        0,
        0,
        0,
      );

      // niko
      let graphics_push_constants = GraphicsPushConstants {
        matrix: matrices.niko,
        tex_offset: data.sprite_buffers.texture_offsets.niko.offset,
        tex_size: data.sprite_buffers.texture_offsets.niko.size,
      };
      device.cmd_push_constants(
        cb,
        pipeline.layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        utility::any_as_u8_slice(&graphics_push_constants),
      );
      device.cmd_draw_indexed(
        cb,
        data.sprite_buffers.models.niko.indices_len as u32,
        1,
        data.sprite_buffers.models.niko.indices_offset as u32,
        data.sprite_buffers.models.niko.vertices_offset as i32,
        0,
      );

      // kakyoin
      let graphics_push_constants = GraphicsPushConstants {
        matrix: matrices.kakyoin,
        tex_offset: data.sprite_buffers.texture_offsets.kakyoin.offset,
        tex_size: data.sprite_buffers.texture_offsets.kakyoin.size,
      };
      device.cmd_push_constants(
        cb,
        pipeline.layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        utility::any_as_u8_slice(&graphics_push_constants),
      );
      device.cmd_draw_indexed(
        cb,
        data.sprite_buffers.models.kakyoin.indices_len as u32,
        1,
        data.sprite_buffers.models.kakyoin.indices_offset as u32,
        data.sprite_buffers.models.kakyoin.vertices_offset as i32,
        0,
      );

      // draw text ui 2d sprite on screen
      if draw_text {
        {
          let ratio_x = data.text_ui_size.width as f32 / RENDER_SIZE.x;
          let ratio_y = data.text_ui_size.height as f32 / RENDER_SIZE.y;
          let offset_pixels_x = 10.0;
          let offset_pixels_y = 10.0;
          // top left + pixels
          let offset_x = -1.0 + ratio_x + (offset_pixels_x * 2.0 / RENDER_SIZE.x);
          let offset_y = -1.0 + ratio_y + (offset_pixels_y * 2.0 / RENDER_SIZE.y);

          // column major orthogonal projection with translation
          // (image on screen)
          let matrix = Matrix4 {
            x: Vector4::new(ratio_x, 0.0, 0.0, 0.0),
            y: Vector4::new(0.0, ratio_y, 0.0, 0.0),
            z: Vector4::new(0.0, 0.0, 0.0, 0.0),
            w: Vector4::new(offset_x, offset_y, 0.0, 1.0),
          };

          let pc = GraphicsPushConstants {
            matrix,
            tex_offset: [0.0, 0.0],
            tex_size: [
              data.text_ui_size.width as f32,
              data.text_ui_size.height as f32,
            ],
          };

          device.cmd_bind_descriptor_sets(
            cb,
            vk::PipelineBindPoint::GRAPHICS,
            pipeline.layout,
            0,
            &[descriptor_pool.text_ui_set],
            &[],
          );
          device.cmd_push_constants(
            cb,
            pipeline.layout,
            vk::ShaderStageFlags::VERTEX,
            0,
            utility::any_as_u8_slice(&pc),
          );
          device.cmd_draw_indexed(
            cb,
            data.sprite_buffers.models.quad.indices_len as u32,
            1,
            0,
            0,
            0,
          );
        }

        {
          let text_pc = SlugPushConstants::new_2d(
            RENDER_SIZE.x,
            RENDER_SIZE.y,
            [10.0, data.text_ui_line_size + 10.0],
          );

          device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::GRAPHICS, text_pipeline.current);
          device.cmd_bind_descriptor_sets(
            cb,
            vk::PipelineBindPoint::GRAPHICS,
            text_pipeline.layout,
            0,
            &[descriptor_pool.text_set],
            &[],
          );
          device.cmd_push_constants(
            cb,
            text_pipeline.layout,
            vk::ShaderStageFlags::VERTEX,
            0,
            utility::any_as_u8_slice(&text_pc),
          );
          // this should be synchronized with host because of host write ordering guarantees
          // https://docs.vulkan.org/spec/latest/chapters/synchronization.html#synchronization-submission-host-writes
          device.cmd_bind_vertex_buffers(cb, 0, &[*data.text.buffers.host[frame_i].vertices], &[0]);
          device.cmd_bind_index_buffer(
            cb,
            *data.text.buffers.host[frame_i].indices,
            0,
            vk::IndexType::UINT32,
          );
          device.cmd_draw_indexed(cb, data.text.host_index_count, 1, 0, 0, 0);
        }
      }

      device.cmd_end_rendering(cb);
    }

    // wait to be ready to copy on render target
    // change layout to transfer_src
    {
      let wait_render_target = vk::ImageMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
        dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
        src_stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        dst_stage_mask: vk::PipelineStageFlags2::COPY.bitor(vk::PipelineStageFlags2::BLIT),
        old_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        image: render_targets.color_images[frame_i],
        subresource_range: ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_RANGE,
        ..Default::default()
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[wait_render_target]));
    }

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
        subresource_range: ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_RANGE,
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
        &[ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_RANGE],
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
        render_targets.color_images[frame_i],
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
        render_targets.color_images[frame_i],
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
        render_targets.color_images[frame_i],
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
        subresource_range: ONE_LAYER_COLOR_IMAGE_SUBRESOURCE_RANGE,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(
        cb,
        &dependency_info(&[], &[], &[swapchain_presentation_layout]),
      );
    }
  }

  pub fn end_recording(&self, device: &ash::Device) -> Result<(), OutOfMemoryError> {
    unsafe {
      device
        .end_command_buffer(self.main)
        .map_err(|err| err.into())
    }
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
  let (dst_start, dst_end) = match width_diff.cmp(&height_diff) {
    Ordering::Greater => {
      // clamp to height
      let ratio = dst_height as f32 / src_height as f32;
      let resized_width = (src_width as f32 * ratio) as i32;

      let half = (dst_width - resized_width) / 2;
      ([half, 0], [half + resized_width, dst_height])
    }
    Ordering::Equal => ([0, 0], [dst_width, dst_height]),
    Ordering::Less => {
      // clamp to width
      let ratio = dst_width as f32 / src_width as f32;
      let resized_height = (src_height as f32 * ratio) as i32;

      let half = (dst_height - resized_height) / 2;
      ([0, half], [dst_width, half + resized_height])
    }
  };
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
