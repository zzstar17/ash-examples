use std::{marker::PhantomData, ptr};

use ash::vk;

use crate::{
  device::QueueFamilies, device_destroyable::DeviceManuallyDestroyed, errors::OutOfMemoryError,
  IMAGE_COLOR,
};

use super::dependency_info;

pub struct ComputeCommandBufferPool {
  pool: vk::CommandPool,
  pub clear_img: vk::CommandBuffer,
}

impl ComputeCommandBufferPool {
  pub fn create(
    device: &ash::Device,
    queue_families: &QueueFamilies,
  ) -> Result<Self, OutOfMemoryError> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(device, flags, queue_families.compute.index)?;

    let clear_img = super::allocate_primary_command_buffers(device, pool, 1)?[0];

    Ok(Self { pool, clear_img })
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), OutOfMemoryError> {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .map_err(|err| err.into())
  }

  pub unsafe fn record_clear_img(
    &mut self,
    device: &ash::Device,
    queue_families: &QueueFamilies,
    image: vk::Image,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.clear_img;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    // image has 1 mip_level / 1 array layer
    let subresource_range = vk::ImageSubresourceRange {
      aspect_mask: vk::ImageAspectFlags::COLOR,
      base_mip_level: 0,
      level_count: 1,
      base_array_layer: 0,
      layer_count: 1,
    };

    let prepare_image = vk::ImageMemoryBarrier2 {
      s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
      p_next: ptr::null(),
      src_access_mask: vk::AccessFlags2::NONE,
      dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
      src_stage_mask: vk::PipelineStageFlags2::NONE,
      dst_stage_mask: vk::PipelineStageFlags2::CLEAR,
      old_layout: vk::ImageLayout::UNDEFINED,
      new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
      image,
      subresource_range,
      _marker: PhantomData,
    };
    device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[prepare_image]));

    device.cmd_clear_color_image(
      cb,
      image,
      vk::ImageLayout::TRANSFER_DST_OPTIMAL,
      &IMAGE_COLOR,
      &[subresource_range],
    );

    let compute_family = queue_families.compute.index;
    let transfer_family = queue_families
      .transfer
      .unwrap_or(queue_families.compute)
      .index;
    if compute_family != transfer_family {
      // Release image to transfer queue family and change image layout at the same time
      // Even though the layout transition operation is submitted twice, it only executes once in
      // between queue ownership transfer
      // https://docs.vulkan.org/spec/latest/chapters/synchronization.html#synchronization-queue-transfers
      let release = vk::ImageMemoryBarrier2 {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_stage_mask: vk::PipelineStageFlags2::CLEAR, // complete clear before transfer
        dst_stage_mask: vk::PipelineStageFlags2::TRANSFER, // to semaphore
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE, // flush copy clear operation
        dst_access_mask: vk::AccessFlags2::NONE,        // NONE for ownership release
        old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        src_queue_family_index: compute_family,
        dst_queue_family_index: transfer_family,
        image,
        subresource_range,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[release]));
    } else {
      // if queues are equal just change image layout
      let change_layout = vk::ImageMemoryBarrier2 {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER_2,
        p_next: ptr::null(),
        src_stage_mask: vk::PipelineStageFlags2::CLEAR,
        dst_stage_mask: vk::PipelineStageFlags2::TRANSFER,
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::TRANSFER_READ,
        old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        new_layout: vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image,
        subresource_range,
        _marker: PhantomData,
      };
      device.cmd_pipeline_barrier2(cb, &dependency_info(&[], &[], &[change_layout]));
    }

    device.end_command_buffer(self.clear_img)?;

    Ok(())
  }
}

impl DeviceManuallyDestroyed for ComputeCommandBufferPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
