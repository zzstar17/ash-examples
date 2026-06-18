use ash::vk;
use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};
use vkobjects::{
  errors::OutOfMemoryError, fill_destroyable_array_with_expression, utility::OnErr,
  DeviceManuallyDestroyed, ManuallyDestroyed,
};

use crate::{
  compute::{gpu_data::ComputeGPUData, sync_renderer::COMPUTE_FRAMES_IN_FLIGHT, ParticleBuffers},
  render::{
    command_pools::{ComputeCommandBufferPool, ComputeTransferCommandBufferPool},
    descriptor_sets::ComputeDescriptorPool,
    pipelines::ComputePipeline,
    InitializationError,
  },
};

pub struct ComputeRenderer {
  pub device: Device,
  pub physical_device: PhysicalDevice,
  pub queues: SingleQueues,

  pub gpu_data: ComputeGPUData,
  pub descriptor_sets: ComputeDescriptorPool,
  pub pipeline: ComputePipeline,

  pub transfer_pool: ComputeTransferCommandBufferPool,
  pub command_pools: [ComputeCommandBufferPool; COMPUTE_FRAMES_IN_FLIGHT],
}

impl ComputeRenderer {
  pub fn new(
    device: Device,
    physical_device: PhysicalDevice,
    queues: SingleQueues,
    particle_buffers: [vk::Buffer; ParticleBuffers::BUFFER_COUNT],
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, InitializationError> {
    let gpu_data = ComputeGPUData::new(&device, &physical_device, particle_buffers, marker)?;
    let descriptor_pool = ComputeDescriptorPool::new(&device).on_err(|_err| unsafe {
      gpu_data.destroy_self(&device);
    })?;
    descriptor_pool.update_initial_sets(
      &device,
      gpu_data.particles_compute,
      gpu_data.particles_new,
    );

    let pipeline = ComputePipeline::new(&device, vk::PipelineCache::null(), &descriptor_pool)
      .on_err(|_err| unsafe {
        descriptor_pool.destroy_self(&device);
        gpu_data.destroy_self(&device);
      })?;

    let transfer_pool = ComputeTransferCommandBufferPool::new(
      &device,
      &queues,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_err| unsafe {
      pipeline.destroy_self(&device);
      descriptor_pool.destroy_self(&device);
      gpu_data.destroy_self(&device);
    })?;

    let command_pools = fill_destroyable_array_with_expression!(
      &device,
      ComputeCommandBufferPool::new(
        &device,
        queues.compute.family_index,
        #[cfg(feature = "vl")]
        marker,
      ),
      COMPUTE_FRAMES_IN_FLIGHT
    )
    .on_err(|_err| unsafe {
      transfer_pool.destroy_self(&device);
      pipeline.destroy_self(&device);
      descriptor_pool.destroy_self(&device);
      gpu_data.destroy_self(&device);
    })?;

    Ok(Self {
      device,
      physical_device,
      queues,
      gpu_data,
      descriptor_sets: descriptor_pool,
      pipeline,
      transfer_pool,
      command_pools,
    })
  }

  pub unsafe fn record_main(
    &mut self,
    read_i: usize,
    write_i: usize,
    particle_buffer_i: Option<usize>,
    write_to_cpu: bool,
  ) -> Result<(), OutOfMemoryError> {
    self.command_pools[write_i].reset(&self.device)?;
    self.command_pools[write_i].record_main(
      read_i,
      write_i,
      &self.device,
      &self.queues,
      &self.gpu_data,
      &self.descriptor_sets,
      &self.pipeline,
      particle_buffer_i,
      write_to_cpu,
    )?;
    Ok(())
  }
}

impl ManuallyDestroyed for ComputeRenderer {
  unsafe fn destroy_self(&self) {
    log::debug!("Destroying ComputeRenderer");
    unsafe {
      let device = &self.device;
      self.transfer_pool.destroy_self(device);
      self.command_pools.destroy_self(device);
      self.pipeline.destroy_self(device);
      self.descriptor_sets.destroy_self(device);
      self.gpu_data.destroy_self(device);
    }
  }
}
