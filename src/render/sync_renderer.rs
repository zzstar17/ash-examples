use std::{marker::PhantomData, ptr};

use ash::vk;
use winit::window::Window;

use crate::{
  compute::ComputeThread, render::create_objs::create_fence, utility::OnErr,
  DEBUG_PRINT_FRAME_INFO, SCREENSHOT_SAVE_FILE,
};

use super::{
  create_objs::create_semaphore,
  device_destroyable::{fill_destroyable_array_with_expression, DeviceManuallyDestroyed},
  errors::InitializationError,
  renderer::Renderer,
  FrameRenderError, FRAMES_IN_FLIGHT,
};

pub struct SyncRenderer {
  pub renderer: Renderer,
  // none if thread has terminated
  compute_thread: Option<ComputeThread>,

  last_frame_i: usize,
  // frame resources are free
  frame_fences: [vk::Fence; FRAMES_IN_FLIGHT],

  // swapchain image available in this frame
  image_available: [vk::Semaphore; FRAMES_IN_FLIGHT],

  // will have the new window size
  recreate_swapchain_next_frame: bool,

  save_next_frame: bool,
  saving_frame: Option<(usize, vk::Format)>, // Some((frame_i, save_format)) if frame's screenshot is being saved
}

impl SyncRenderer {
  pub fn new(renderer: Renderer) -> Result<Self, InitializationError> {
    let device = &renderer.device;
    let compute_thread = crate::compute::start_compute(device.inner.clone());
    let frame_fences = fill_destroyable_array_with_expression!(
      device,
      create_fence(
        device,
        vk::FenceCreateFlags::SIGNALED,
        #[cfg(feature = "vl")]
        &renderer.debug_utils_marker,
        #[cfg(feature = "vl")]
        c"frame fence"
      ),
      FRAMES_IN_FLIGHT
    )?;

    let image_available = fill_destroyable_array_with_expression!(
      &renderer.device,
      create_semaphore(
        device,
        #[cfg(feature = "vl")]
        &renderer.debug_utils_marker,
        #[cfg(feature = "vl")]
        c"image available"
      ),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { frame_fences.destroy_self(device) })?;

    Ok(Self {
      renderer,
      compute_thread: Some(compute_thread),
      last_frame_i: FRAMES_IN_FLIGHT - 1, // 1 so that the first frame starts at 0
      frame_fences,

      image_available,
      recreate_swapchain_next_frame: false,
      save_next_frame: false,
      saving_frame: None,
    })
  }

  pub fn window_resized(&mut self) {
    self.recreate_swapchain_next_frame = true;
  }

  pub fn window(&self) -> &Window {
    &self.renderer.window
  }

  pub fn render_next_frame(&mut self, cur_total_frame: usize) -> Result<(), FrameRenderError> {
    let cur_frame_i = (self.last_frame_i + 1) % FRAMES_IN_FLIGHT;
    self.last_frame_i = cur_frame_i;

    // wait for frame of the same set (that holds current frame resources) to finish rendering
    unsafe {
      self
        .renderer
        .device
        .wait_for_fences(&[self.frame_fences[cur_frame_i]], true, u64::MAX)?;
    }

    // current frame resources are now safe to use as they are not being used by the GPU

    let destroyed_old_swapchain = self
      .renderer
      .swapchains
      .attempt_destroy_old(&self.renderer.device, cur_total_frame)?;
    if destroyed_old_swapchain {
      unsafe {
        self.renderer.cleanup_after_old_swapchain(cur_total_frame);
      }
    }

    if self.recreate_swapchain_next_frame {
      unsafe {
        self.renderer.recreate_swapchain(cur_total_frame)?;
      }
      self.recreate_swapchain_next_frame = false;
    }

    if let Some((frame, format)) = self.saving_frame {
      if frame == cur_frame_i {
        self.saving_frame = None;
        match self.renderer.save_screenshot_buffer_as_rgba8(format) {
          Ok(()) => {
            println!(
              "[Frame {}] Screenshot saved to {:?}",
              cur_total_frame, SCREENSHOT_SAVE_FILE
            );
          }
          Err(err) => {
            log::error!(
              "Failed to save screenshot to {:?}:\n{:?}",
              SCREENSHOT_SAVE_FILE,
              err
            );
          }
        }
      }
    }

    let (cur_image_i, image_finished_presenting) = match unsafe {
      self
        .renderer
        .swapchains
        .acquire_next_image(self.image_available[cur_frame_i])
    } {
      Ok((image_index, suboptimal, image_finished_presenting)) => {
        if suboptimal {
          self.recreate_swapchain_next_frame = true;
        }
        (image_index, image_finished_presenting)
      }
      Err(err) => {
        log::warn!(
          "[Frame {}] Failed to acquire next swapchain image",
          cur_total_frame
        );
        self.recreate_swapchain_next_frame = true;

        return Err(err.into());
      }
    };

    if DEBUG_PRINT_FRAME_INFO {
      log::debug!(
        "Rendering new frame. Image: {}, Frame (group): {}",
        cur_image_i,
        cur_frame_i
      );
    }

    unsafe {
      // only reset after making sure the fence is going to be signalled again
      self
        .renderer
        .device
        .reset_fences(&[self.frame_fences[cur_frame_i]])?;
    }

    // get compute data

    let ferris_render_position = self
      .compute_thread
      .as_ref()
      .unwrap()
      .result_receiver
      .recv()
      .expect("Compute thread data sender has disconnected");

    // actual rendering

    unsafe {
      let mut record_screenshot = false;
      if self.save_next_frame && self.saving_frame.is_none() {
        self.save_next_frame = false;
        self.saving_frame = Some((cur_frame_i, self.renderer.render_format()));
        record_screenshot = true;
      }
      self.renderer.record_graphics(
        cur_frame_i,
        cur_image_i as usize,
        &ferris_render_position,
        record_screenshot,
      )?;
    }

    let command_buffers = [vk::CommandBufferSubmitInfo::default()
      .command_buffer(self.renderer.command_pools[cur_frame_i].main)];

    let wait_semaphores = [
      // wait for image to become ready for writes
      // the stage_mask will be synched with any dependencies existing in the command buffer
      // recording
      vk::SemaphoreSubmitInfo {
        s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
        p_next: ptr::null(),
        semaphore: self.image_available[cur_frame_i],
        value: 0, // ignored
        // stage where the swapchain image stops being used by the presentation operation
        // see notes in https://docs.vulkan.org/spec/latest/chapters/synchronization.html#synchronization-semaphores-waiting
        stage_mask: vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
        device_index: 0, // ignored
        _marker: PhantomData,
      },
    ];

    let signal_semaphores = [
      // when can the presentation operation start using the image
      vk::SemaphoreSubmitInfo {
        s_type: vk::StructureType::SEMAPHORE_SUBMIT_INFO,
        p_next: ptr::null(),
        semaphore: image_finished_presenting,
        value: 0, // ignored
        // last stages that affect the current swapchain image
        stage_mask: vk::PipelineStageFlags2::ALL_COMMANDS,
        device_index: 0, // ignored
        _marker: PhantomData,
      },
    ];
    let submit_info = vk::SubmitInfo2::default()
      .command_buffer_infos(&command_buffers)
      .wait_semaphore_infos(&wait_semaphores)
      .signal_semaphore_infos(&signal_semaphores);
    unsafe {
      self.renderer.device.queue_submit2(
        self.renderer.queues.graphics.handle,
        &[submit_info],
        self.frame_fences[cur_frame_i],
      )?;
    }

    unsafe {
      if let Err(err) = self.renderer.swapchains.queue_present(
        &self.renderer.device,
        cur_image_i,
        self.renderer.queues.graphics.handle,
        &[image_finished_presenting],
      ) {
        self.recreate_swapchain_next_frame = true;
        return Err(err.into());
      }
    }

    Ok(())
  }

  pub fn screenshot(&mut self) {
    if self.save_next_frame {
      println!("New screenshot failed, currently processing previous screenshot request");
    } else {
      self.save_next_frame = true;
    }
  }
}

impl Drop for SyncRenderer {
  fn drop(&mut self) {
    // waits for device
    let compute_thread = self.compute_thread.take();
    compute_thread.unwrap().terminate_and_wait();

    log::debug!("Destroying SyncRenderer");
    let device = &self.renderer.device;
    unsafe {
      self.frame_fences.destroy_self(device);
      self.image_available.destroy_self(device);
    }
  }
}
