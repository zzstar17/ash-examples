use std::ops::BitOr;

use ash::vk;
use vkinitialization::device::SingleQueues;
use vkobjects::{errors::OutOfMemoryError, utility, DeviceManuallyDestroyed};

use crate::{
  RESOLUTION, compute::ComputeGPUData, render::{
    descriptor_sets::ComputeDescriptorPool,
    pipelines::{ComputePipeline, ComputePushConstants},
    vertices::Particle,
  }
};

pub struct ComputeCommandBufferPool {
  pool: vk::CommandPool,
  pub cb: vk::CommandBuffer,
}

impl ComputeCommandBufferPool {
  pub fn new(
    device: &ash::Device,
    queue_family_index: u32,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, OutOfMemoryError> {
    let flags = vk::CommandPoolCreateFlags::TRANSIENT;
    let pool = super::create_command_pool(
      device,
      flags,
      queue_family_index,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"compute",
    )?;

    let command_buffers = super::allocate_primary_command_buffers(
      device,
      pool,
      1,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      &[c"compute"],
    )?;
    let cb = command_buffers[0];

    Ok(Self { pool, cb })
  }

  pub unsafe fn record_main(
    &self,
    read_i: usize,
    write_i: usize,
    device: &ash::Device,
    queues: &SingleQueues,

    data: &ComputeGPUData,
    descriptors: &ComputeDescriptorPool,
    pipeline: &ComputePipeline,

    particle_buffer_i_opt: Option<usize>,
    write_to_cpu: bool,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.cb;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    let new_particles_count = data.particles_copying;
    let push_constants = ComputePushConstants {
      render_dimensions: [RESOLUTION[0] as f32, RESOLUTION[1] as f32],
      player_pos: [0.0, 0.0],
      particle_count: data.particles_len as u32,
      new_particle_count: new_particles_count,
    };
    let new_particles_size = new_particles_count as u64 * size_of::<Particle>() as u64;

    if new_particles_count > 0 {
      if queues.transfer.family_index != queues.compute.family_index {
        let acquire = vk::BufferMemoryBarrier2 {
          src_access_mask: vk::AccessFlags2::empty(), // ownership acquire
          dst_access_mask: vk::AccessFlags2::SHADER_READ,
          src_stage_mask: vk::PipelineStageFlags2::empty(), // ownership acquire
          dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
          src_queue_family_index: queues.transfer.family_index,
          dst_queue_family_index: queues.compute.family_index,
          buffer: data.particles_new,
          offset: 0,
          size: new_particles_size,
          ..Default::default()
        };
        device.cmd_pipeline_barrier2(cb, &super::dependency_info(&[], &[acquire], &[]));
      } else {
        let copy_wait = vk::BufferMemoryBarrier2 {
          src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
          dst_access_mask: vk::AccessFlags2::SHADER_READ,
          src_stage_mask: vk::PipelineStageFlags2::COPY,
          dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
          src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
          dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
          buffer: data.particles_new,
          offset: 0,
          size: new_particles_size,
          ..Default::default()
        };
        device.cmd_pipeline_barrier2(cb, &super::dependency_info(&[], &[copy_wait], &[]));
      }
    }

    // wait previous dispatch
    let cur_buffer_size = data.current_particles_size();
    if cur_buffer_size > 0 {
      let read_wait = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::SHADER_WRITE.bitor(vk::AccessFlags2::SHADER_READ),
        dst_access_mask: vk::AccessFlags2::SHADER_READ,
        src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        buffer: data.particles_compute[read_i],
        offset: 0,
        size: cur_buffer_size,
        ..Default::default()
      };
      let write_wait = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::SHADER_WRITE.bitor(vk::AccessFlags2::SHADER_READ),
        dst_access_mask: vk::AccessFlags2::SHADER_WRITE.bitor(vk::AccessFlags2::TRANSFER_READ),
        src_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER,
        dst_stage_mask: vk::PipelineStageFlags2::COMPUTE_SHADER
          .bitor(vk::PipelineStageFlags2::TRANSFER),
        buffer: data.particles_compute[write_i],
        offset: 0,
        size: cur_buffer_size,
        ..Default::default()
      };
      device.cmd_pipeline_barrier2(
        cb,
        &super::dependency_info(&[], &[read_wait, write_wait], &[]),
      );
    }

    // shader dispatch
    {
      let new_particles_descriptor = if new_particles_count > 0 {
        log::debug!("Binding particles new");
        descriptors.particles_new
      } else {
        descriptors.particles_compute[read_i]
      };

      device.cmd_bind_descriptor_sets(
        cb,
        vk::PipelineBindPoint::COMPUTE,
        pipeline.layout,
        0,
        // read, write, new
        &[
          descriptors.particles_compute[read_i],
          descriptors.particles_compute[write_i],
          new_particles_descriptor,
        ],
        &[],
      );
      device.cmd_push_constants(
        cb,
        pipeline.layout,
        vk::ShaderStageFlags::COMPUTE,
        0,
        utility::any_as_u8_slice(&push_constants),
      );
      device.cmd_bind_pipeline(cb, vk::PipelineBindPoint::COMPUTE, pipeline.main);

      // todo
      let group_count = ComputeGPUData::INITIAL_CAPACITY as u32;
      device.cmd_dispatch(cb, group_count, 1, 1);
    }

    if let Some(particle_buffer_i) = particle_buffer_i_opt {
      // in case previous write was last frame
      let wait_previous_write = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::COPY,
        buffer: data.particles_graphics[particle_buffer_i],
        offset: 0,
        size: cur_buffer_size,
        ..Default::default()
      };
      device.cmd_pipeline_barrier2(
        cb,
        &super::dependency_info(&[], &[wait_previous_write], &[]),
      );

      let region = vk::BufferCopy {
        src_offset: 0,
        dst_offset: 0,
        size: cur_buffer_size,
      };
      device.cmd_copy_buffer(
        cb,
        data.particles_compute[read_i],
        data.particles_graphics[particle_buffer_i],
        &[region],
      );

      if queues.compute.family_index != queues.graphics.family_index {
        let release_to_graphics = vk::BufferMemoryBarrier2 {
          src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
          dst_access_mask: vk::AccessFlags2::empty(), // ownership release
          src_stage_mask: vk::PipelineStageFlags2::COPY,
          dst_stage_mask: vk::PipelineStageFlags2::empty(), // ownership release
          src_queue_family_index: queues.compute.family_index,
          dst_queue_family_index: queues.graphics.family_index,
          buffer: data.particles_graphics[particle_buffer_i],
          offset: 0,
          size: cur_buffer_size,
          ..Default::default()
        };
        device.cmd_pipeline_barrier2(
          cb,
          &super::dependency_info(&[], &[release_to_graphics], &[]),
        );
      }
    }

    if write_to_cpu {
      log::debug!("Command buffer compute writing to cpu");
      let region = vk::BufferCopy {
        src_offset: 0,
        dst_offset: 0,
        size: cur_buffer_size,
      };
      device.cmd_copy_buffer(
        cb,
        data.particles_compute[read_i],
        data.to_cpu_write.buffer,
        &[region],
      );

      let flush_to_host = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::HOST_READ,
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::HOST,
        buffer: data.to_cpu_write.buffer,
        offset: 0,
        size: cur_buffer_size,
        ..Default::default()
      };
      // sync with next same write
      let flush_to_next_copy_write = vk::BufferMemoryBarrier2 {
        src_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        dst_access_mask: vk::AccessFlags2::TRANSFER_WRITE,
        src_stage_mask: vk::PipelineStageFlags2::COPY,
        dst_stage_mask: vk::PipelineStageFlags2::COPY,
        buffer: data.to_cpu_write.buffer,
        offset: 0,
        size: cur_buffer_size,
        ..Default::default()
      };
      device.cmd_pipeline_barrier2(
        cb,
        &super::dependency_info(&[], &[flush_to_host, flush_to_next_copy_write], &[]),
      );
    }

    device.end_command_buffer(cb)?;
    Ok(())
  }

  pub unsafe fn reset(&mut self, device: &ash::Device) -> Result<(), OutOfMemoryError> {
    device
      .reset_command_pool(self.pool, vk::CommandPoolResetFlags::empty())
      .map_err(|err| err.into())
  }
}

// should not be called while buffer is in submission
impl DeviceManuallyDestroyed for ComputeCommandBufferPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
