use std::{ptr, sync::mpsc, time::Duration};

use ash::vk;
use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};
use vkobjects::{
  errors::OutOfMemoryError, utility::OnErr, DeviceManuallyDestroyed, ManuallyDestroyed,
};
use winit::dpi::PhysicalSize;

use crate::{
  compute::{
    particle_buffers::ParticleManager, renderer::ComputeRenderer, ComputeGPUData, ComputeResult,
    ParticleBuffers,
  },
  render::{
    create_objs::{create_fence, create_semaphore},
    InitializationError, RENDER_EXTENT,
  },
  RESOLUTION,
};

use super::ferris::Ferris;

pub const COMPUTE_FRAMES_IN_FLIGHT: usize = 2;

pub struct ComputeSyncRenderer {
  tick_i: usize,

  ferris: Ferris,

  compute_result_sender: mpsc::SyncSender<ComputeResult>,
  particle_manager: ParticleManager,

  renderer: ComputeRenderer,

  last_write_i: usize,
  frame_fences: [vk::Fence; COMPUTE_FRAMES_IN_FLIGHT],

  transfer_finished: vk::Semaphore,

  save_gpu_contents_next_frame: bool,
  saving_gpu_contents: Option<u32>,
}

#[derive(Debug, thiserror::Error)]
pub enum ComputeFrameRenderError {
  #[error("Graphics thread receiver disconnected")]
  SenderDisconnected,

  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),

  #[error("Device is lost")]
  DeviceLost,
}

impl From<vk::Result> for ComputeFrameRenderError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        ComputeFrameRenderError::OutOfMemory(OutOfMemoryError::from(value))
      }
      vk::Result::ERROR_DEVICE_LOST => ComputeFrameRenderError::DeviceLost,
      _ => panic!("Invalid cast from vk::Result to ComputeFrameRenderError"),
    }
  }
}

impl ComputeSyncRenderer {
  pub fn new(
    device: Device,
    physical_device: PhysicalDevice,
    queues: SingleQueues,
    compute_result_sender: mpsc::SyncSender<ComputeResult>,
    particle_buffers: ParticleBuffers,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, InitializationError> {
    let ferris = Ferris::new([0.2, 0.0], true, true);

    // todo: write all on errors
    let mut renderer = ComputeRenderer::new(
      device,
      physical_device,
      queues,
      particle_buffers.buffers,
      #[cfg(feature = "vl")]
      marker,
    )?;

    let transfer_finished = create_semaphore(
      &renderer.device,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Compute transfer",
    )?;
    let frame0 = create_fence(
      &renderer.device,
      vk::FenceCreateFlags::SIGNALED,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Compute main 0",
    )?;
    let frame1 = create_fence(
      &renderer.device,
      vk::FenceCreateFlags::SIGNALED,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Compute main 1",
    )?;
    let frame_fences = [frame0, frame1];

    // write initial data to gpu_data
    renderer
      .gpu_data
      .write_particles_to_from_cpu_read(&renderer.device, ComputeGPUData::INITIAL_CAPACITY)?;

    unsafe {
      renderer.transfer_pool.record_copy_particles_new(
        &renderer.device,
        &queues,
        &renderer.gpu_data,
        renderer.gpu_data.current_new_particles_size(),
      )?;
      let submit_info = vk::SubmitInfo {
        wait_semaphore_count: 0,
        p_wait_semaphores: ptr::null(),
        p_wait_dst_stage_mask: ptr::null(),
        command_buffer_count: 1,
        p_command_buffers: &renderer.transfer_pool.copy_particles_new,
        signal_semaphore_count: 1,
        p_signal_semaphores: &transfer_finished,
        ..Default::default()
      };

      renderer
        .device
        .queue_submit(queues.transfer.handle, &[submit_info], vk::Fence::null())?;
    }

    let particle_manager = ParticleManager::new(particle_buffers.in_use_by_graphics);

    Ok(Self {
      tick_i: 0,
      renderer,
      ferris,
      compute_result_sender,
      transfer_finished,
      frame_fences,
      last_write_i: COMPUTE_FRAMES_IN_FLIGHT - 1,
      save_gpu_contents_next_frame: true,
      saving_gpu_contents: None,
      particle_manager,
    })
  }

  pub fn next_compute_frame(
    &mut self,
    time_since_last_update: Duration,
  ) -> Result<(), ComputeFrameRenderError> {
    let cur_read_i = self.last_write_i;
    let cur_write_i = (self.last_write_i + 1) % COMPUTE_FRAMES_IN_FLIGHT;
    self.last_write_i = cur_write_i;

    // wait for frame of the same set (that holds current frame resources) to finish rendering
    unsafe {
      self
        .renderer
        .device
        .wait_for_fences(&[self.frame_fences[cur_write_i]], true, u64::MAX)?;
    }
    // particles buffer can be written to again even if it was written to last compute frame
    // (as long as it is not being used by graphics)
    self.particle_manager.compute_finished();

    if self.tick_i < 10 {
      self.save_gpu_contents_next_frame = true;
      log::warn!(
        "[Tick {}] Particle count: {}, new particles: {}",
        self.tick_i,
        self.renderer.gpu_data.particles_len,
        self.renderer.gpu_data.particles_copying
      );
      log::warn!(
        "[Tick {}]\nParticles: {:?}",
        self.tick_i,
        self.particle_manager
      );
    }

    if let Some(count) = self.saving_gpu_contents {
      let contents = unsafe {
        self
          .renderer
          .gpu_data
          .to_cpu_write
          .read_to_box(count as usize)
      };
      log::warn!(
        "[Tick {}] Compute contents: {:?} * {}",
        self.tick_i,
        contents[0],
        contents.len()
      );
    }
    self.saving_gpu_contents = None;

    if self.renderer.gpu_data.particles_len == 0 {
      self.save_gpu_contents_next_frame = false;
    }

    let particles_write_i = self.particle_manager.get_next_compute_i();

    if self.tick_i < 10 {
      log::warn!(
        "[Tick {}]\nParticles: {:?}",
        self.tick_i,
        self.particle_manager
      );
    }

    unsafe {
      self
        .renderer
        .record_main(cur_read_i, cur_write_i, self.save_gpu_contents_next_frame)
    }
    .on_err(|_err| self.particle_manager.compute_fail())?;

    unsafe {
      // only reset after making sure the fence is going to be signalled again
      self
        .renderer
        .device
        .reset_fences(&[self.frame_fences[cur_write_i]])
    }
    .on_err(|_err| self.particle_manager.compute_fail())?;

    let command_buffers = [vk::CommandBufferSubmitInfo::default()
      .command_buffer(self.renderer.command_pools[cur_write_i].cb)];

    let particle_copy_wait = [vk::SemaphoreSubmitInfo {
      semaphore: self.transfer_finished,
      stage_mask: vk::PipelineStageFlags2::TRANSFER,
      ..Default::default()
    }];
    let empty: [vk::SemaphoreSubmitInfo<'_>; 0] = [];

    let wait_semaphores: &[vk::SemaphoreSubmitInfo<'_>] =
      if self.renderer.gpu_data.particles_copying > 0 {
        &particle_copy_wait
      } else {
        &empty
      };

    let submit_info = vk::SubmitInfo2::default()
      .command_buffer_infos(&command_buffers)
      .wait_semaphore_infos(&wait_semaphores);
    unsafe {
      self.renderer.device.queue_submit2(
        self.renderer.queues.compute.handle,
        &[submit_info],
        self.frame_fences[cur_write_i],
      )
    }
    .on_err(|_err| self.particle_manager.compute_fail())?;
    self.tick_i += 1;

    if self.save_gpu_contents_next_frame {
      // before commit
      self.saving_gpu_contents = Some(self.renderer.gpu_data.particles_len);
      self.save_gpu_contents_next_frame = false;
    }

    if self.renderer.gpu_data.particles_copying > 0 {
      self.renderer.gpu_data.commit_new_particles();
    }

    self.ferris.update(
      time_since_last_update,
      PhysicalSize {
        width: RESOLUTION[0],
        height: RESOLUTION[1],
      },
    );

    // todo: unmark this when send is full
    let graphics_buffer_read_i_opt = self.particle_manager.get_and_mark_next_graphics();

    if let Some(graphics_buffer_read_i) = graphics_buffer_read_i_opt {
      let render_position = self.ferris.get_render_position(PhysicalSize {
        width: RENDER_EXTENT.width,
        height: RENDER_EXTENT.height,
      });

      let compute_result = ComputeResult {
        ferris_position: render_position,
        particle_buffer_i: graphics_buffer_read_i,
      };

      match self.compute_result_sender.try_send(compute_result) {
        Ok(()) => {}
        Err(err) => {
          self
            .particle_manager
            .unmark_graphics(graphics_buffer_read_i);
          match err {
            mpsc::TrySendError::Full(_) => {}
            mpsc::TrySendError::Disconnected(_) => {
              return Err(ComputeFrameRenderError::SenderDisconnected);
            }
          }
        }
      }
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

      let device = &self.renderer.device;
      self.frame_fences.destroy_self(device);
      self.transfer_finished.destroy_self(device);
      ManuallyDestroyed::destroy_self(&self.renderer);
    }
  }
}
