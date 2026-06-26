use std::{
  marker::PhantomData,
  ptr,
  sync::{atomic::Ordering, mpsc},
};

use ash::vk;
use vkobjects::{fill_destroyable_array_with_expression, utility::OnErr, DeviceManuallyDestroyed};
use winit::window::Window;

use crate::{
  render::{
    compute::ComputeFrameResult,
    create_objs::{create_fence, create_semaphore},
    graphics, FrameRenderError, InitializationError, GRAPHICS_FRAMES_IN_FLIGHT,
  },
  DEBUG_PRINT_FRAME_INFO, SCREENSHOT_SAVE_FILE,
};

pub struct SyncRenderer {
  pub renderer: graphics::Renderer,

  last_frame_i: usize,
  // frame resources are free
  frame_fences: [vk::Fence; GRAPHICS_FRAMES_IN_FLIGHT],

  // swapchain image available in this frame
  image_available: [vk::Semaphore; GRAPHICS_FRAMES_IN_FLIGHT],

  in_use_particle_buffers_by_frame: [Option<usize>; GRAPHICS_FRAMES_IN_FLIGHT],

  // will have the new window size
  recreate_swapchain_next_frame: bool,

  save_next_frame: bool,
  saving_frame: Option<(usize, vk::Format)>, // Some((frame_i, save_format)) if frame's screenshot is being saved
}

impl SyncRenderer {
  pub fn new(renderer: graphics::Renderer) -> Result<Self, InitializationError> {
    let device = &renderer.init.device;
    let frame_fences = fill_destroyable_array_with_expression!(
      device,
      create_fence(
        device,
        vk::FenceCreateFlags::SIGNALED,
        #[cfg(feature = "vl")]
        &renderer.init.debug_utils_marker,
        #[cfg(feature = "vl")]
        c"frame fence"
      ),
      GRAPHICS_FRAMES_IN_FLIGHT
    )?;

    let image_available = fill_destroyable_array_with_expression!(
      &renderer.init.device,
      create_semaphore(
        device,
        #[cfg(feature = "vl")]
        &renderer.init.debug_utils_marker,
        #[cfg(feature = "vl")]
        c"image available"
      ),
      GRAPHICS_FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { frame_fences.destroy_self(device) })?;

    Ok(Self {
      renderer,
      last_frame_i: GRAPHICS_FRAMES_IN_FLIGHT - 1, // 1 so that the first frame starts at 0
      frame_fences,
      in_use_particle_buffers_by_frame: [None; GRAPHICS_FRAMES_IN_FLIGHT],

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
    &self.renderer.init.window
  }

  pub fn render_next_frame(
    &mut self,
    cur_total_frame: usize,
    compute_message_rcv: &mpsc::Receiver<ComputeFrameResult>,
  ) -> Result<(), FrameRenderError> {
    let cur_frame_i = (self.last_frame_i + 1) % GRAPHICS_FRAMES_IN_FLIGHT;
    self.last_frame_i = cur_frame_i;

    // wait for frame of the same set (that holds current frame resources) to finish rendering
    unsafe {
      self.renderer.init.device.wait_for_fences(
        &[self.frame_fences[cur_frame_i]],
        true,
        u64::MAX,
      )?;
    }
    if let Some(buffer_i) = self.in_use_particle_buffers_by_frame[cur_frame_i] {
      self.renderer.particle_buffers.in_use_by_graphics[buffer_i].store(false, Ordering::Release);
    }

    // current frame resources are now safe to use as they are not being used by the GPU

    let destroyed_old_swapchain = self
      .renderer
      .swapchains
      .attempt_destroy_old(&self.renderer.init.device, cur_total_frame)?;
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
        .init
        .device
        .reset_fences(&[self.frame_fences[cur_frame_i]])?;
    }

    // get compute data

    let ComputeFrameResult { particles_draw } = compute_message_rcv
      .recv()
      .map_err(|_err| FrameRenderError::ComputeThreadDisconnected)?;

    // actual rendering

    unsafe {
      let mut record_screenshot = false;
      if self.save_next_frame && self.saving_frame.is_none() {
        self.save_next_frame = false;
        self.saving_frame = Some((cur_frame_i, self.renderer.render_format()));
        record_screenshot = true;
      }
      self
        .renderer
        .record_graphics(
          cur_frame_i,
          cur_image_i as usize,
          particles_draw,
          record_screenshot,
        )
        .on_err(|_err| {
          self.renderer.particle_buffers.in_use_by_graphics[particles_draw.buffer_i]
            .store(false, Ordering::Release);
        })?;
    }

    // commit in_use_by_graphics
    self.in_use_particle_buffers_by_frame[cur_frame_i] = Some(particles_draw.buffer_i);

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
      self.renderer.init.device.queue_submit2(
        self.renderer.init.queues.graphics.handle,
        &[submit_info],
        self.frame_fences[cur_frame_i],
      )?;
    }

    unsafe {
      if let Err(err) = self.renderer.swapchains.queue_present(
        &self.renderer.init.device,
        cur_image_i,
        self.renderer.init.queues.graphics.handle,
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

  pub unsafe fn destroy_self(&mut self) {
    let device = &self.renderer.init.device;
    unsafe {
      self.frame_fences.destroy_self(device);
      self.image_available.destroy_self(device);

      self.renderer.destroy_self();
    }
  }
}
