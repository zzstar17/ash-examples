use std::{sync::mpsc, thread};

use winit::{event_loop::ActiveEventLoop, window::Window};

use crate::{
  compute::{ComputeFrameResult, ComputeToGraphicsEvent, GraphicsToComputeEvent},
  render::{FrameRenderError, InitializationError, PostWindowInit, Renderer, SyncRenderer},
};

pub struct ComputeThreadData {
  pub handle: thread::JoinHandle<()>,
  pub result_receiver: mpsc::Receiver<ComputeFrameResult>,
  pub event_sender: mpsc::Sender<GraphicsToComputeEvent>,
  pub event_receiver: mpsc::Receiver<Result<ComputeToGraphicsEvent, InitializationError>>,
}

impl ComputeThreadData {
  pub fn terminate_and_wait(self) {
    if let Err(_err) = self.event_sender.send(GraphicsToComputeEvent::Terminate) {
      log::warn!("Failed to send termination event to compute thread. Receiver is disconnected.")
    }
    if let Err(_err) = self.handle.join() {
      log::error!("Compute thread panicked.");
    }
  }
}

pub struct ThreadsManager {
  // none if thread has terminated
  compute_thread_data: Option<ComputeThreadData>,

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
    )?;

    let compute_thread_data = ComputeThreadData {
      handle: compute_thread.handle,
      result_receiver: compute_thread.result_receiver,
      event_sender: compute_thread.event_sender,
      event_receiver: compute_thread.event_receiver,
    };
    let particle_buffers = compute_thread.particle_buffers;

    let renderer = Renderer::initialize(post_window_init, particle_buffers)?;
    let mut sync_renderer = SyncRenderer::new(renderer)?;

    let receiver_res = compute_thread_data.event_receiver.recv();
    let mut compute_initialized = false;
    match receiver_res {
      Ok(event_res) => match event_res {
        Ok(event) => match event {
          ComputeToGraphicsEvent::InitializationComplete => {
            log::info!("Compute thread initialization complete");
            compute_initialized = true;
          }
        },
        Err(err) => {
          log::error!("Compute thread failed to initialize.\n{}", err);
        }
      },
      Err(_err) => {
        log::error!("Compute thread disconnected before even finishing initializing");
      }
    }
    if !compute_initialized {
      unsafe {
        sync_renderer.destroy_self();
      }
    }

    Ok(Self {
      compute_thread_data: Some(compute_thread_data),
      graphics_render: sync_renderer,
    })
  }

  pub fn render_next_frame(&mut self, cur_total_frame: usize) -> Result<(), FrameRenderError> {
    let compute_thread = self.compute_thread_data.as_ref().unwrap();
    let compute_message_rcv = &compute_thread.result_receiver;

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
    let compute_thread = self.compute_thread_data.take();
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
