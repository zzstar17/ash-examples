pub mod ferris;
mod gpu_data;
mod renderer;
mod sync_renderer;

pub use gpu_data::ComputeGPUData;

use std::{
  sync::mpsc::{self},
  thread,
  time::{Duration, Instant},
};

pub use sync_renderer::ComputeSyncRenderer;
use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};

use crate::{
  compute::sync_renderer::ComputeFrameRenderError,
  last_frames_durations::LastFramesDurations,
  render::{InitializationError, RenderPosition},
  KEEP_FRAME_DURATION_COUNT_UPS, MAX_UPS, PRINT_UPS_EVERY,
};

pub enum GraphicsToComputeEvent {
  Terminate,
}

pub enum ComputeToGraphicsEvent {
  InitializationComplete,
}

pub struct ComputeThread {
  pub handle: thread::JoinHandle<()>,
  pub result_receiver: mpsc::Receiver<RenderPosition>,
  pub event_sender: mpsc::Sender<GraphicsToComputeEvent>,
  pub event_receiver: mpsc::Receiver<Result<ComputeToGraphicsEvent, InitializationError>>,
}

impl ComputeThread {
  pub fn terminate_and_wait(self) {
    if let Err(_err) = self.event_sender.send(GraphicsToComputeEvent::Terminate) {
      log::warn!("Failed to send termination event to compute thread. Receiver is disconnected.")
    }
    if let Err(_err) = self.handle.join() {
      log::error!("Compute thread panicked.");
    }
  }
}

pub fn start_compute(
  device: Device,
  physical_device: PhysicalDevice,
  queues: SingleQueues,
  #[cfg(feature = "vl")] marker: vkinitialization::DebugUtilsMarker,
) -> ComputeThread {
  let (data_sender, data_receiver) = mpsc::sync_channel(1);
  // events from compute queue
  let (compute_event_sender, compute_event_receiver) = mpsc::channel();
  // events from graphics queue
  let (graphics_event_sender, graphics_event_receiver) = mpsc::channel();

  let handle = thread::spawn(move || {
    log::info!("Starting compute thread");

    let mut sync_renderer =
      match ComputeSyncRenderer::new(device, physical_device, queues, data_sender, &marker) {
        Ok(v) => {
          if let Err(_err) =
            compute_event_sender.send(Ok(ComputeToGraphicsEvent::InitializationComplete))
          {
            log::error!("Main thread disconnected during initialization");
            return;
          }
          v
        }
        Err(err) => {
          match compute_event_sender.send(Err(err)) {
            Ok(()) => {}
            Err(_err) => {
              log::error!("Main thread disconnected during initialization");
            }
          }
          return;
        }
      };

    let mut last_update = Instant::now();
    let mut time_since_last_ups_print = Duration::ZERO;
    let mut last_frames_durations: LastFramesDurations<KEEP_FRAME_DURATION_COUNT_UPS> =
      LastFramesDurations::new();
    let min_time_between_frames = Duration::from_secs_f64(1.0 / MAX_UPS);

    'compute_loop: loop {
      for event in graphics_event_receiver.try_iter() {
        match event {
          GraphicsToComputeEvent::Terminate => {
            break 'compute_loop;
          }
        }
      }

      let mut now = Instant::now();
      let mut time_passed = now - last_update;
      if time_passed < min_time_between_frames {
        thread::sleep(min_time_between_frames - time_passed);

        now += min_time_between_frames - time_passed; // general estimate
        time_passed = now - last_update
      }
      last_update = now;

      last_frames_durations.update_new(time_passed);

      time_since_last_ups_print += time_passed;
      if time_since_last_ups_print >= PRINT_UPS_EVERY {
        time_since_last_ups_print -= PRINT_UPS_EVERY;
        let (min, max, average) = last_frames_durations.get_min_max_average_fps();
        println!("UPS: {:.4} {:.4} {:.4}", min, max, average);
      }

      if let Err(err) = sync_renderer.next_compute_frame(time_passed) {
        match err {
          ComputeFrameRenderError::SenderDisconnected => {
            log::error!("Main thread disconnected");
            break;
          }
        }
      }
    }

    log::info!("Compute thead: Exiting");
  });

  ComputeThread {
    handle,
    result_receiver: data_receiver,
    event_sender: graphics_event_sender,
    event_receiver: compute_event_receiver,
  }
}
