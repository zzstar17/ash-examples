use winit::{event_loop::ActiveEventLoop, window::Window};

use crate::{
  compute::ComputeThread,
  render::{FrameRenderError, InitializationError, PostWindowInit, Renderer, SyncRenderer},
};

pub struct ThreadsManager {
  // none if thread has terminated
  compute_thread: Option<ComputeThread>,

  pub graphics_render: SyncRenderer,
}

impl ThreadsManager {
  pub fn start(
    pre_window: super::PreWindowInit,
    event_loop: &ActiveEventLoop,
  ) -> Result<Self, InitializationError> {
    let post_window_init = PostWindowInit::initialize(pre_window, event_loop)?;

    let compute_thread = crate::compute::start_compute(
      post_window_init.device.clone(),
      post_window_init.physical_device.clone(),
      post_window_init.queues.clone(),
      post_window_init.debug_utils_marker.clone(),
    );

    let renderer = Renderer::initialize(post_window_init)?;
    let sync_renderer = SyncRenderer::new(renderer)?;

    Ok(Self {
      compute_thread: Some(compute_thread),
      graphics_render: sync_renderer,
    })
  }

  pub fn render_next_frame(&mut self, cur_total_frame: usize) -> Result<(), FrameRenderError> {
    let compute_message_rcv = &self.compute_thread.as_ref().unwrap().result_receiver;

    self
      .graphics_render
      .render_next_frame(cur_total_frame, compute_message_rcv)?;

    Ok(())
  }

  pub fn window(&self) -> &Window {
    self.graphics_render.window()
  }

  pub fn window_resized(&mut self) {
    self.graphics_render.window_resized();
  }

  pub fn screenshot(&mut self) {
    self.graphics_render.screenshot();
  }
}

impl Drop for ThreadsManager {
  fn drop(&mut self) {
    // waits for device
    let compute_thread = self.compute_thread.take();
    compute_thread.unwrap().terminate_and_wait();

    unsafe {
      self
        .graphics_render
        .renderer
        .init
        .device
        .device_wait_idle()
        .expect("Failed to wait for device idle");
      self.graphics_render.destroy_self();
    }
  }
}
