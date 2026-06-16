use ash::vk;
use vkinitialization::device::SingleQueues;
use vkobjects::{errors::OutOfMemoryError, DeviceManuallyDestroyed};

use crate::compute::ComputeGPUData;

pub struct ComputeTransferCommandBufferPool {
  pool: vk::CommandPool,
  pub copy_particles_new: vk::CommandBuffer,
}

impl ComputeTransferCommandBufferPool {
  pub fn new(
    device: &ash::Device,
    queues: &SingleQueues,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, OutOfMemoryError> {
    let flags = vk::CommandPoolCreateFlags::empty();
    let pool = super::create_command_pool(
      device,
      flags,
      queues.transfer.family_index,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"transfer compute",
    )?;

    let command_buffers = super::allocate_primary_command_buffers(
      device,
      pool,
      1,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      &[c"copy_particles_new"],
    )?;
    let copy_particles_new = command_buffers[0];

    Ok(Self {
      pool,
      copy_particles_new,
    })
  }

  pub unsafe fn record_copy_particles_new(
    &self,
    device: &ash::Device,
    queues: &SingleQueues,

    data: &ComputeGPUData,
    new_particles_size: u64,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.copy_particles_new;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    {
      let region = vk::BufferCopy {
        src_offset: 0,
        dst_offset: 0,
        size: new_particles_size,
      };
      device.cmd_copy_buffer(
        cb,
        data.particles_from_cpu_read.buffer,
        data.particles_new,
        &[region],
      );
    }

    if queues.transfer.family_index != queues.compute.family_index {
      let release_to_compute = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::empty(), // ownership release
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::empty(), // ownership release
        src_queue_family_index: queues.transfer.family_index,
        dst_queue_family_index: queues.compute.family_index,
        buffer: data.particles_new,
        offset: 0,
        size: new_particles_size,
        ..Default::default()
      };
      device.cmd_pipeline_barrier2(cb, &super::dependency_info(&[], &[release_to_compute], &[]));
    }

    device.end_command_buffer(cb)?;
    Ok(())
  }
}

// should not be called while buffer is in submission
impl DeviceManuallyDestroyed for ComputeTransferCommandBufferPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
