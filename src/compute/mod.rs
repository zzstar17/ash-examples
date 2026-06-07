pub mod ferris;
mod gpu_data;
mod sync_renderer;

use std::{
  sync::mpsc::{self},
  thread,
  time::{Duration, Instant},
};

pub use sync_renderer::ComputeSyncRenderer;
use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};

use crate::{
  last_frames_durations::LastFramesDurations, render::RenderPosition,
  KEEP_FRAME_DURATION_COUNT_UPS, MAX_UPS, PRINT_UPS_EVERY,
};

pub enum ComputeEvent {
  Terminate,
}

pub struct ComputeThread {
  pub handle: thread::JoinHandle<()>,
  pub result_receiver: mpsc::Receiver<RenderPosition>,
  pub event_sender: mpsc::Sender<ComputeEvent>,
}

impl ComputeThread {
  pub fn terminate_and_wait(self) {
    self
      .event_sender
      .send(ComputeEvent::Terminate)
      .expect("Failed to send termination event to compute thread. Receiver is disconnected.");
    self
      .handle
      .join()
      .expect("Compute thread finished abruptly");
  }
}

pub fn start_compute(
  device: Device,
  physical_device: PhysicalDevice,
  queues: SingleQueues,
  #[cfg(feature = "vl")] marker: vkinitialization::DebugUtilsMarker,
) -> ComputeThread {
  let (data_sender, data_receiver) = mpsc::sync_channel(1);
  let (event_sender, event_receiver) = mpsc::channel();

  let handle = thread::spawn(move || {
    log::info!("Starting compute thread");

    // todo: handle errors
    let mut sync_renderer =
      ComputeSyncRenderer::new(device, &physical_device, &queues, data_sender, &marker)
        .expect("Failed to start compute sync renderer");

    let mut last_update = Instant::now();
    let mut time_since_last_ups_print = Duration::ZERO;
    let mut last_frames_durations: LastFramesDurations<KEEP_FRAME_DURATION_COUNT_UPS> =
      LastFramesDurations::new();
    let min_time_between_frames = Duration::from_secs_f64(1.0 / MAX_UPS);

    'compute_loop: loop {
      for event in event_receiver.try_iter() {
        match event {
          ComputeEvent::Terminate => {
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

      sync_renderer.next_compute_frame(time_passed);
    }

    log::info!("Compute thead: Exiting");
  });

  ComputeThread {
    handle,
    result_receiver: data_receiver,
    event_sender: event_sender,
  }
}
