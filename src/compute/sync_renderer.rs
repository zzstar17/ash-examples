use std::{marker::PhantomData, ptr, sync::mpsc, time::Duration};

use ash::vk;
use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};
use vkobjects::{DeviceManuallyDestroyed, ManuallyDestroyed};
use winit::dpi::PhysicalSize;

use crate::{
  compute::renderer::ComputeRenderer,
  render::{create_objs::create_fence, InitializationError, RenderPosition, RENDER_EXTENT},
  RESOLUTION,
};

use super::ferris::Ferris;

pub struct ComputeSyncRenderer {
  ferris: Ferris,

  compute_result_sender: mpsc::SyncSender<RenderPosition>,

  renderer: ComputeRenderer,

  instance_compute_fences: [vk::Fence; 2],
}

#[derive(Debug, thiserror::Error)]
pub enum ComputeFrameRenderError {
  #[error("Graphics thread receiver disconnected")]
  SenderDisconnected,
}

impl ComputeSyncRenderer {
  pub fn new(
    device: Device,
    physical_device: PhysicalDevice,
    queues: SingleQueues,
    compute_result_sender: mpsc::SyncSender<RenderPosition>,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, InitializationError> {
    let ferris = Ferris::new([0.2, 0.0], true, true);

    let renderer = ComputeRenderer::new(
      device,
      physical_device,
      queues,
      #[cfg(feature = "vl")]
      marker,
    )?;

    let instance_compute0_fence = create_fence(
      &renderer.device,
      vk::FenceCreateFlags::empty(),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Instance compute 0",
    )?;
    let instance_compute1_fence = create_fence(
      &renderer.device,
      vk::FenceCreateFlags::empty(),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Instance compute 1",
    )?;
    let instance_compute_fences = [instance_compute0_fence, instance_compute1_fence];

    // write initial data to gpu_data
    renderer
      .gpu_data
      .initialize_particles_compute(&renderer.device)?;

    unsafe {
      renderer.record_initialization()?;
      let submit_info = vk::SubmitInfo {
        s_type: vk::StructureType::SUBMIT_INFO,
        p_next: ptr::null(),
        wait_semaphore_count: 0,
        p_wait_semaphores: ptr::null(),
        p_wait_dst_stage_mask: ptr::null(),
        command_buffer_count: 1,
        p_command_buffers: &renderer.command_pool.cb,
        signal_semaphore_count: 0,
        p_signal_semaphores: ptr::null(),
        _marker: PhantomData,
      };

      renderer.device.queue_submit(
        queues.compute.handle,
        &[submit_info],
        instance_compute0_fence,
      )?;
      renderer
        .device
        .wait_for_fences(&[instance_compute0_fence], true, u64::MAX)?;
    }

    Ok(Self {
      renderer,
      ferris,
      compute_result_sender,
      instance_compute_fences,
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
        .renderer
        .device
        .device_wait_idle()
        .expect("Failed to wait for device idleness while dropping Compute SyncRenderer");

      self
        .instance_compute_fences
        .destroy_self(&self.renderer.device);
      ManuallyDestroyed::destroy_self(&self.renderer);
    }
  }
}
