use ash::vk;
use vkobjects::{errors::OutOfMemoryError, DeviceManuallyDestroyed};

use crate::compute::ComputeGPUData;

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
    device: &ash::Device,

    data: &ComputeGPUData,
    copy_size: u64,
  ) -> Result<(), OutOfMemoryError> {
    let cb = self.cb;
    let begin_info =
      vk::CommandBufferBeginInfo::default().flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    device.begin_command_buffer(cb, &begin_info)?;

    {
      let region = vk::BufferCopy {
        src_offset: 0,
        dst_offset: 0,
        size: copy_size,
      };
      device.cmd_copy_buffer(
        self.cb,
        data.particles_from_cpu_read.buffer,
        data.particles_compute[0],
        &[region],
      );
    }

    device.end_command_buffer(cb)?;
    Ok(())
  }
}

// should not be called while buffer is in submission
impl DeviceManuallyDestroyed for ComputeCommandBufferPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_command_pool(self.pool, None);
  }
}
