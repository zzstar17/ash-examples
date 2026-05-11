use std::{marker::PhantomData, ptr};

use ash::{vk, Device};
use vkobjects::{
  errors::{OutOfMemoryError, QueueSubmitError},
  DeviceManuallyDestroyed,
};

use crate::create_objs::create_fence;

pub struct InitCommandBufferPool {
  pool: vk::CommandPool,
  cb: vk::CommandBuffer,
}

#[must_use]
#[derive(Debug)]
pub struct PendingInitialization {
  pool: vk::CommandPool,
  fence: vk::Fence,
}

impl PendingInitialization {
  pub unsafe fn wait_and_self_destroy(&self, device: &ash::Device) -> Result<(), QueueSubmitError> {
    device.wait_for_fences(&[self.fence], true, u64::MAX)?;

    self.fence.destroy_self(device);
    self.pool.destroy_self(device);
    Ok(())
  }
}

impl InitCommandBufferPool {
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
      c"initialization",
    )?;

    let command_buffers = super::allocate_primary_command_buffers(
      device,
      pool,
      1,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      &[c"initialization"],
    )?;
    let cb = command_buffers[0];
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    unsafe {
      device.begin_command_buffer(cb, &begin_info)?;
    }

    Ok(Self { pool, cb })
  }

  pub unsafe fn record_copy_staging_buffer_to_buffer(
    &self,
    device: &ash::Device,
    staging: vk::Buffer,
    dst: vk::Buffer,
    size: vk::DeviceSize,
  ) {
    let region = vk::BufferCopy {
      src_offset: 0,
      dst_offset: 0,
      size,
    };
    device.cmd_copy_buffer(self.cb, staging, dst, &[region]);
  }

  pub unsafe fn end_and_submit(
    self,
    device: &Device,
    queue: vk::Queue,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<PendingInitialization, (Self, QueueSubmitError)> {
    let cb: vk::CommandBuffer = self.cb;
    if let Err(err) = device.end_command_buffer(cb) {
      return Err((self, err.into()));
    }

    let fence = match create_fence(
      device,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"initialization",
    ) {
      Ok(v) => v,
      Err(err) => return Err((self, err.into())),
    };
    let submit_info = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: &cb,
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
      _marker: PhantomData,
    };
    unsafe {
      if let Err(err) = device.queue_submit(queue, &[submit_info], fence) {
        return Err((self, err.into()));
      }
    }
    Ok(PendingInitialization {
      pool: self.pool,
      fence,
    })
  }
}

// should not be called while buffer is in submission
impl DeviceManuallyDestroyed for InitCommandBufferPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
