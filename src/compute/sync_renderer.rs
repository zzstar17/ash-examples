use std::{sync::mpsc, time::Duration};

use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};
use vkobjects::DeviceManuallyDestroyed;
use winit::dpi::PhysicalSize;

use crate::{
  compute::gpu_data::GPUData,
  render::{InitializationError, RenderPosition, RENDER_EXTENT},
  RESOLUTION,
};

use super::ferris::Ferris;

pub struct ComputeSyncRenderer {
  device: Device,

  ferris: Ferris,

  compute_result_sender: mpsc::SyncSender<RenderPosition>,

  gpu_data: GPUData,
}

#[derive(Debug, thiserror::Error)]
pub enum ComputeFrameRenderError {
  #[error("Graphics thread receiver disconnected")]
  SenderDisconnected,
}

impl ComputeSyncRenderer {
  pub fn new(
    device: Device,
    physical_device: &PhysicalDevice,
    queues: &SingleQueues,
    compute_result_sender: mpsc::SyncSender<RenderPosition>,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, InitializationError> {
    let ferris = Ferris::new([0.2, 0.0], true, true);

    let gpu_data = GPUData::new(&device, physical_device, queues, marker)?;

    Ok(Self {
      device,
      ferris,
      compute_result_sender,
      gpu_data,
    })
  }

  pub fn next_compute_frame(
    &mut self,
    time_since_last_update: Duration,
  ) -> Result<(), ComputeFrameRenderError> {
    self.ferris.update(
      time_since_last_update,
      PhysicalSize {
        width: RESOLUTION[0],
        height: RESOLUTION[1],
      },
    );

    let render_position = self.ferris.get_render_position(PhysicalSize {
      width: RENDER_EXTENT.width,
      height: RENDER_EXTENT.height,
    });

    match self.compute_result_sender.try_send(render_position) {
      Ok(()) => {}
      Err(err) => match err {
        mpsc::TrySendError::Full(_) => {}
        mpsc::TrySendError::Disconnected(_) => {
          return Err(ComputeFrameRenderError::SenderDisconnected);
        }
      },
    }

    Ok(())
  }
}

impl Drop for ComputeSyncRenderer {
  fn drop(&mut self) {
    log::debug!("Destroying ComputeSyncRenderer");
    unsafe {
      self
        .device
        .device_wait_idle()
        .expect("Failed to wait for device idleness while dropping Compute SyncRenderer");

      self.gpu_data.destroy_self(&self.device);
    }
  }
}
