use std::{
  sync::{mpsc, Arc, RwLock},
  thread,
};

use vkobjects::{utility::OnErr, DeviceManuallyDestroyed};
use winit::{
  dpi::PhysicalPosition, event::ElementState, event_loop::ActiveEventLoop, window::Window,
};

use crate::{
  last_frames_durations::FPSDurations,
  render::{
    compute::{self, ComputeFrameResult, ComputeToGraphicsEvent, GraphicsToComputeEvent},
    format_conversions::{self, KNOWN_FORMATS},
    gpu_data::{sprite_buffers::SpriteTextureData, GPUData},
    graphics, FrameRenderError, InitializationError, PostWindowInit,
  },
  WindowToComputeInfo,
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

  pub graphics_render: graphics::SyncRenderer,
}

impl ThreadsManager {
  pub fn start(
    pre_window: super::PreWindowInit,
    event_loop: &ActiveEventLoop,
    window_info: Arc<RwLock<WindowToComputeInfo>>,
  ) -> Result<Self, InitializationError> {
    let post_window_init = PostWindowInit::initialize(pre_window, event_loop)?;

    let mut sprite_data = SpriteTextureData::read_texture_bytes_as_rgba8()?;

    let swapchain_format = post_window_init.swapchains.get_format();
    let texture_format = if KNOWN_FORMATS.contains(&swapchain_format) {
      swapchain_format
    } else {
      KNOWN_FORMATS
        .into_iter()
        .find(|&f| {
          crate::render::initialization::format_is_supported(
            &post_window_init.instance,
            *post_window_init.physical_device,
            f,
          )
        })
        .unwrap()
    };

    format_conversions::convert_rgba_data_to_format(&mut sprite_data.bytes, texture_format);
    log::info!("Creating texture with the format {:?}", texture_format);

    let gpu_data = GPUData::new(
      &post_window_init.device,
      &post_window_init.physical_device,
      texture_format,
      &sprite_data,
      #[cfg(feature = "vl")]
      &post_window_init.debug_utils_marker,
    )?;

    let compute_thread = compute::start_compute(
      post_window_init.device.clone(),
      post_window_init.physical_device.clone(),
      post_window_init.queues,
      window_info,
      &gpu_data,
      #[cfg(feature = "vl")]
      post_window_init.debug_utils_marker.clone(),
    )
    .on_err(|_err| unsafe { gpu_data.destroy_self(&post_window_init.device) })?;

    let compute_thread_data = ComputeThreadData {
      handle: compute_thread.handle,
      result_receiver: compute_thread.result_receiver,
      event_sender: compute_thread.event_sender,
      event_receiver: compute_thread.event_receiver,
    };
    let particle_buffers = compute_thread.particle_buffers;

    // takes ownership of gpu_data
    let renderer = graphics::Renderer::initialize(post_window_init, particle_buffers, gpu_data)?;
    let mut sync_renderer = graphics::SyncRenderer::new(renderer, &sprite_data)?;

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

  pub fn render_next_frame(
    &mut self,
    cur_total_frame: usize,
    fps: FPSDurations,
  ) -> Result<(), FrameRenderError> {
    let compute_thread = self.compute_thread_data.as_ref().unwrap();
    let compute_message_rcv = &compute_thread.result_receiver;

    self
      .graphics_render
      .render_next_frame(cur_total_frame, compute_message_rcv, fps)?;

    Ok(())
  }

  pub fn mouse_click(
    &self,
    pressed: ElementState,
    position: PhysicalPosition<f64>,
  ) -> Result<(), mpsc::SendError<GraphicsToComputeEvent>> {
    self
      .compute_thread_data
      .as_ref()
      .unwrap()
      .event_sender
      .send(GraphicsToComputeEvent::MouseClick((pressed, position)))
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
