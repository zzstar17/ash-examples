use ash::vk;
use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};
use vkobjects::{
  errors::OutOfMemoryError, utility::OnErr, DeviceManuallyDestroyed, ManuallyDestroyed,
};

use crate::{
  compute::gpu_data::ComputeGPUData,
  render::{
    command_pools::ComputeCommandBufferPool, descriptor_sets::ComputeDescriptorPool,
    pipelines::ComputePipeline, InitializationError,
  },
};

pub struct ComputeRenderer {
  pub device: Device,
  pub physical_device: PhysicalDevice,
  pub queues: SingleQueues,

  pub gpu_data: ComputeGPUData,
  pub descriptor_sets: ComputeDescriptorPool,
  pub pipeline: ComputePipeline,

  pub command_pool: ComputeCommandBufferPool,
}

impl ComputeRenderer {
  pub fn new(
    device: Device,
    physical_device: PhysicalDevice,
    queues: SingleQueues,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, InitializationError> {
    let gpu_data = ComputeGPUData::new(&device, &physical_device, &queues, marker)?;
    let descriptor_pool = ComputeDescriptorPool::new(&device).on_err(|_err| unsafe {
      gpu_data.destroy_self(&device);
    })?;
    descriptor_pool.update_initial_sets(
      &device,
      gpu_data.particles_compute,
      gpu_data.particles_compute[1],
    );

    let pipeline = ComputePipeline::new(&device, vk::PipelineCache::null(), &descriptor_pool)
      .on_err(|_err| unsafe {
        descriptor_pool.destroy_self(&device);
        gpu_data.destroy_self(&device);
      })?;

    let command_pool = ComputeCommandBufferPool::new(
      &device,
      queues.compute.family_index,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_err| unsafe {
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
      command_pool,
    })
  }

  pub unsafe fn record_initialization(&self) -> Result<(), OutOfMemoryError> {
    self
      .command_pool
      .record_main(&self.device, &self.gpu_data, self.gpu_data.particles_size)
  }
}

impl ManuallyDestroyed for ComputeRenderer {
  unsafe fn destroy_self(&self) {
    log::debug!("Destroying ComputeRenderer");
    unsafe {
      let device = &self.device;
      self.command_pool.destroy_self(device);
      self.pipeline.destroy_self(device);
      self.descriptor_sets.destroy_self(device);
      self.gpu_data.destroy_self(device);
    }
  }
}
