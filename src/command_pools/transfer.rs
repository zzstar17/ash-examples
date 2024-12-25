use std::{marker::PhantomData, ptr};

use ash::vk;
use ash_destructor::DeviceDestroyable;

use crate::{device::QueueFamilies, errors::OutOfMemoryError, IMAGE_HEIGHT, IMAGE_WIDTH};

use super::dependency_info;

#[derive(DeviceDestroyable)]
pub struct TransferCommandBufferPool {
  pool: vk::CommandPool,
  #[destroy_ignore]
  pub copy_image_to_buffer: vk::CommandBuffer,
}

impl TransferCommandBufferPool {
  pub fn create(device: &ash::Device, queue_families: &QueueFamilies) -> Result<Self, vk::Result> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.get_transfer_index())?;

    let copy_image_to_buffer = super::allocate_primary_command_buffers(device, pool, 1)?[0];

    Ok(Self {
      pool,
      copy_image_to_buffer,
    })
  }

  pub unsafe fn reset(&self, device: &ash::Device) -> Result<(), vk::Result> {
    device.reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
  }

  pub unsafe fn record_copy_img_to_buffer(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    src_image: vk::Image,
    dst_buffer: vk::Buffer,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.copy_image_to_buffer;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    if queue_families.get_compute_index() != queue_families.get_transfer_index() {
      // matches to release found in compute
      let src_acquire = vk::ImageMemoryBarrier2 {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_access_mask: vk::AccessFlags2::NONE, // NONE for ownership acquire,
        dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
        src_stage_mask: vk::PipelineStageFlags2::TRANSFER, // from semaphore
        dst_stage_mask: vk::PipelineStageFlags2::COPY,
        old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        src_queue_family_index: queue_families.get_compute_index(),
        dst_queue_family_index: queue_families.get_transfer_index(),
        image: src_image,
        subresource_range,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[src_acquire]));
    }

    // 1 color layer
    let subresource_layers = vk::ImageSubresourceLayers {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      mip_level: 0,
      base_array_layer: 0,
      layer_count: 1,
    };
    // full image
    let copy_region = vk::BufferImageCopy {
      image_subresource: subresource_layers,
      image_offset: vk::Offset3D { x: 0, y: 0, z: 0 },
      image_extent: vk::Extent3D {
        width: IMAGE_WIDTH,
        height: IMAGE_HEIGHT,
        depth: 1,
      },
      buffer_offset: 0,
      buffer_image_height: 0, // densely packed
      buffer_row_length: 0,
    };
    device.cmd_copy_image_to_buffer(
      cb,
      src_image,
      vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
      dst_buffer,
      &[copy_region],
    );

    // flush memory to host (device writes are not automatically made available)
    // having the buffer reside in memory marked as coherent is not relevant to domain operations
    let flush_host = vk::BufferMemoryBarrier2 {
      s_type: vk::StructureType::BUFFER_MEMORY_BARRIER_2,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
      dst_access_mask: vk::AccessFlags2::HOST_READ,
      src_stage_mask: vk::PipelineStageFlags2::COPY,
      dst_stage_mask: vk::PipelineStageFlags2::HOST,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      buffer: dst_buffer,
      offset: 0,
      size: vk::WHOLE_SIZE,
      _marker: PhantomData,
    };
    device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[flush_host], &[]));

    device.end_command_buffer(cb)?;

    Ok(())
  }
}
