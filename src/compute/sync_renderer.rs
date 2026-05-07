use std::{sync::mpsc, time::Duration};

use winit::dpi::PhysicalSize;

use crate::{
  render::{RenderPosition, RENDER_EXTENT},
  RESOLUTION,
};

use super::ferris::Ferris;

pub struct ComputeSyncRenderer {
  device: ash::Device,

  ferris: Ferris,

  compute_result_sender: mpsc::SyncSender<RenderPosition>,
}

impl ComputeSyncRenderer {
  pub fn new(device: ash::Device, compute_result_sender: mpsc::SyncSender<RenderPosition>) -> Self {
    let ferris = Ferris::new([0.2, 0.0], true, true);

    Self {
      device,
      ferris,
      compute_result_sender,
    }
  }

  pub fn next_compute_frame(&mut self, time_since_last_update: Duration) {
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
          panic!("Compute loop: Render mpsc receiver disconnected")
        }
      },
    }
  }
}

impl Drop for ComputeSyncRenderer {
  fn drop(&mut self) {
    log::debug!("Destroying ComputeSyncRenderer");
    unsafe {
      self
        .device
        .device_wait_idle()
        .expect("Failed to wait for device idleness while dropping SyncRenderer");
    }
  }
}
