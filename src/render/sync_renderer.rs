use std::{marker::PhantomData, ptr};

use ash::vk;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
  ferris::Ferris, render::create_objs::create_fence, utility::OnErr, SCREENSHOT_SAVE_FILE,
};

use super::{
  create_objs::create_semaphore,
  device_destroyable::{destroy, fill_destroyable_array_with_expression, DeviceManuallyDestroyed},
  errors::InitializationError,
  renderer::Renderer,
  FrameRenderError, FRAMES_IN_FLIGHT, RENDER_EXTENT,
};

pub struct SyncRenderer {
  pub renderer: Renderer,
  last_frame_i: usize,
  frame_fences: [vk::Fence; FRAMES_IN_FLIGHT],

  // swapchain semaphores
  image_available: [vk::Semaphore; FRAMES_IN_FLIGHT],
  presentable: [vk::Semaphore; FRAMES_IN_FLIGHT],

  // last frame swapchain was recreated and so current frame resources are marked as old
  // having more than two frames in flight could require having more than one old set of resources
  last_frame_recreated_swapchain: bool,
  // will have the new window size
  recreate_swapchain_next_frame: bool,

  save_next_frame: bool,
  saving_frame: Option<(usize, vk::Format)>, // Some((frame_i, save_format)) if frame's screenshot is being saved
}

impl SyncRenderer {
  pub fn new(renderer: Renderer) -> Result<Self, InitializationError> {
    let device = &renderer.device;
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
    let presentable = fill_destroyable_array_with_expression!(
      &renderer.device,
      create_semaphore(
        device,
        #[cfg(feature = "vl")]
        &renderer.debug_utils_marker,
        #[cfg(feature = "vl")]
        c"image presentable"
      ),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { destroy!(device => frame_fences.as_ref(), image_available.as_ref()) })?;

    Ok(Self {
      renderer,
      last_frame_i: 1, // 1 so that the first frame starts at 0
      frame_fences,

      image_available,
      presentable,

      last_frame_recreated_swapchain: false,
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

  pub fn render_next_frame(&mut self, ferris: &Ferris) -> Result<(), FrameRenderError> {
    // there are two (corresponding to the number of frames in flight) sets of frames
    // in this example each frame set only owns its own graphics command buffer and nothing else, but
    // as a command buffer can only hold the recording of one specific frame, one current frame
    // needs to wait for the previous one of the same set to be able to record its commands.

    // swapchain images indices are not related to the index of the current frame. Each time a
    // frame occurs the swapchain can give any image that will become available.

    // example: given sets A and B one possible situation would be:
    //  frame 0: belongs to set A and it is given a swapchain image index of 0.
    //      Doesn't wait for anything to begin rendering. *
    //  frame 1: belongs to set B and its given a swapchain image index of 1.
    //      Doesn't wait for anything to begin rendering. *
    //  frame 2: belongs to set A and its given a swapchain image index of 2.
    //      Waits for resources belonging to set A. In this case, waits so that frame 0 finishes
    //      and the command buffer becomes available to be rerecorded. *
    //  frame 3: belongs to set B and its given a swapchain image index of 0.
    //      Waits for resources belonging to set B. In this case, waits so that frame 1 finishes
    //      and the command buffer becomes available to be rerecorded. *
    //  ...
    // * Each frame also has to wait for the corresponding image to become truly available, as it
    //  could be still being used in presentation.

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

    if self.last_frame_recreated_swapchain {
      unsafe { self.renderer.destroy_old() }
      self.last_frame_recreated_swapchain = false;
    }

    if self.recreate_swapchain_next_frame {
      unsafe {
        self.renderer.recreate_swapchain()?;
      }
      self.recreate_swapchain_next_frame = false;
      self.last_frame_recreated_swapchain = true;
    }

    if let Some((frame, format)) = self.saving_frame {
      if frame == cur_frame_i {
        self.saving_frame = None;
        match self.renderer.save_screenshot_buffer_as_rgba8(format) {
          Ok(()) => {
            println!("Screenshot saved to {:?}", SCREENSHOT_SAVE_FILE);
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

    let image_index = match unsafe {
      self
        .renderer
        .swapchains
        .acquire_next_image(self.image_available[cur_frame_i])
    } {
      Ok((image_index, suboptimal)) => {
        if suboptimal {
          self.recreate_swapchain_next_frame = true;
        }
        image_index
      }
      Err(err) => {
        log::warn!("Failed to acquire next swapchain image");
        self.recreate_swapchain_next_frame = true;

        return Err(err.into());
      }
    };

    unsafe {
      // only reset after making sure the fence is going to be signalled again
      self
        .renderer
        .device
        .reset_fences(&[self.frame_fences[cur_frame_i]])?;
    }

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
        image_index as usize,
        &ferris.get_render_position(PhysicalSize {
          width: RENDER_EXTENT.width,
          height: RENDER_EXTENT.height,
        }),
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
        semaphore: self.presentable[cur_frame_i],
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
        image_index,
        self.renderer.queues.graphics.handle,
        &[self.presentable[cur_frame_i]],
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
    let device = &self.renderer.device;
    unsafe {
      device
        .device_wait_idle()
        .expect("Failed to wait for device idleness while dropping SyncRenderer");

      self.frame_fences.destroy_self(device);
      self.image_available.destroy_self(device);
      self.presentable.destroy_self(device);
    }
  }
}
