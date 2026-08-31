mod destructor;
mod font;
mod last_frames_durations;
mod render;
mod slug;
mod threads_manager;

use ash::vk;
use render::{FrameRenderError, InitializationError, PreWindowInit, PreWindowInitError};
use std::{
  ffi::CStr,
  sync::{Arc, RwLock},
  time::{Duration, Instant},
};
use winit::{
  application::ApplicationHandler,
  dpi::{PhysicalPosition, PhysicalSize},
  error::EventLoopError,
  event::{DeviceEvent, MouseButton, WindowEvent},
  event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
  keyboard::{KeyCode, PhysicalKey},
};

use crate::{
  last_frames_durations::LastFramesDurations, render::AcquireNextImageError,
  threads_manager::ThreadsManager,
};

const APPLICATION_NAME: &CStr = c"Bouncy Ferris";
const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

const WINDOW_TITLE: &str = "Bouncy Ferris";
const INITIAL_WINDOW_WIDTH: u32 = 800;
const INITIAL_WINDOW_HEIGHT: u32 = 800;

const RESOLUTION: [u32; 2] = [800, 800];

const TEXTURE_PATH: &str = "./sprites.png";

// get first that exists
const TEXT_FONT: [&str; 3] = ["Source Code Pro", "Consolas", "Arial"];

const SCREENSHOT_SAVE_FILE: &str = "last_screenshot.png";

// const BACKGROUND_COLOR: vk::ClearColorValue = vk::ClearColorValue {
//   float32: [0.1, 0.1, 0.1, 1.0],
// };
const BACKGROUND_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.5, 0.5, 0.5, 1.0],
};
// color exterior the game area
// (that appears if window is resized to a size with ratio different that in RESOLUTION)
const OUT_OF_BOUNDS_AREA_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [1.0, 0.0, 0.0, 1.0],
};

// see https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPresentModeKHR.html
// FIFO_KHR is required to be supported and functions as vsync
// IMMEDIATE will be chosen over RELAXED_KHR if the latter is not supported
// otherwise, presentation mode will fallback to FIFO_KHR
const PREFERRED_PRESENTATION_METHOD: vk::PresentModeKHR = vk::PresentModeKHR::IMMEDIATE;

// prints current frame 1 / <time since last frame> every x time
const PRINT_FPS_EVERY: Duration = Duration::from_millis(1000);
// same but for compute frames
const PRINT_UPS_EVERY: Duration = Duration::from_millis(1000);
// keep track of the duration of last x frames
const KEEP_FRAME_DURATION_COUNT_FPS: usize = 16;
const KEEP_FRAME_DURATION_COUNT_UPS: usize = KEEP_FRAME_DURATION_COUNT_FPS;

const MAX_UPS: f64 = 400.0;

const START_PAUSED: bool = false; // start application in a paused state

const RENDER_UNTIL_FRAME: usize = usize::MAX;
// const RENDER_UNTIL_FRAME: usize = 120;

const DEBUG_PRINT_FRAME_INFO: bool = false;
const LOG_SWAPCHAIN_WARNINGS: bool = false;
const ENABLE_USE_DEBUG_SHADERS: bool = false;

// This application doesn't use dynamic pipeline size, so resizing is expensive
// If a small resize happens (for example while resizing with the mouse) this usually means that
// more are to come, and recreating objects each frame can make the application lag
// If enabled, the render function will wait for more window events unless some threshold is passed
const WAIT_FOR_MULTIPLE_RESIZE_EVENTS_ENABLED: bool = false;
const FORCE_WINDOW_RESIZE_SIZE_THRESHOLD: u32 = 20; // how many pixels before forcing update
                                                    // how much time before forcing update
const FORCE_WINDOW_RESIZE_DURATION_THRESHOLD: Duration = Duration::from_millis(60);
struct WindowResizeHandler {
  pub active: bool,
  pub last_activation_instant: Instant,
  pub last_activation_size: PhysicalSize<u32>,
}

// clippy kinda hallucinates here
#[allow(clippy::large_enum_variant)]
enum RenderStatus {
  Initialized(PreWindowInit),
  Started(StartedStatus),
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Default)]
struct RenderAreaDimensions {
  pub render_size: [u32; 2],
  pub apparent_size: [u32; 2],
  pub apparent_ratio: f64,
  pub render_area_window_offset: [u32; 2],
  pub window_size: [u32; 2],
}

impl RenderAreaDimensions {
  pub fn new(window_dimensions: PhysicalSize<u32>) -> Self {
    let window_size = [window_dimensions.width, window_dimensions.height];
    let apparent_ratio = Self::calculate_render_ratio(
      RESOLUTION[0] as f64,
      RESOLUTION[1] as f64,
      window_size[0] as f64,
      window_size[1] as f64,
    );
    let apparent_size = [
      (RESOLUTION[0] as f64 * apparent_ratio) as u32,
      (RESOLUTION[1] as f64 * apparent_ratio) as u32,
    ];
    let render_area_window_offset = [
      (window_size[0] - apparent_size[0]) / 2,
      (window_size[1] - apparent_size[1]) / 2,
    ];

    RenderAreaDimensions {
      render_size: RESOLUTION,
      apparent_size,
      apparent_ratio,
      render_area_window_offset,
      window_size,
    }
  }

  pub fn into_apparent_coordinates(&self, window_coordinates: PhysicalPosition<f64>) -> [f64; 2] {
    let offsetted_x = window_coordinates.x - self.render_area_window_offset[0] as f64;
    let offsetted_y = window_coordinates.y - self.render_area_window_offset[1] as f64;
    let apparent_x = offsetted_x / self.apparent_ratio;
    let apparent_y = offsetted_y / self.apparent_ratio;
    [apparent_x, apparent_y]
  }

  fn calculate_render_ratio(
    render_width: f64,
    render_height: f64,
    window_width: f64,
    window_height: f64,
  ) -> f64 {
    let width_diff = window_width - render_width;
    let height_diff = window_height - render_height;
    if width_diff > height_diff {
      // clamped to height
      window_height / render_height
    } else {
      // clamped to width
      window_width / render_width
    }
  }
}

struct StartedStatus {
  pub threads_manager: ThreadsManager,
  pub paused: bool,
  pub occluded: bool,
  pub suspended: bool,
  pub waiting_for_window_events: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct WindowToComputeInfo {
  pub initialized: bool,
  pub mouse_position: PhysicalPosition<f64>,
  pub render_dimensions: RenderAreaDimensions,
}

struct App {
  status: RenderStatus,
  window_resize_handler: WindowResizeHandler,
  // sent to compute thread
  compute_info: Arc<RwLock<WindowToComputeInfo>>,
  // same as compute_info
  mouse_position: PhysicalPosition<f64>,
  mouse_in_window: bool,
  last_update: Instant,
  time_since_last_fps_print: Duration,
  last_frames_durations: LastFramesDurations<KEEP_FRAME_DURATION_COUNT_FPS>,
  frame_i: usize,
}

impl StartedStatus {
  pub fn should_draw(&self) -> bool {
    !self.paused && !self.occluded && !self.suspended && !self.waiting_for_window_events
  }

  // set control flow to poll if frames are ok to draw
  fn update_control_flow(&self, event_loop: &ActiveEventLoop) {
    if self.should_draw() {
      event_loop.set_control_flow(ControlFlow::Poll);
    } else {
      if self.waiting_for_window_events {
        if let Some(until) = Instant::now().checked_add(FORCE_WINDOW_RESIZE_DURATION_THRESHOLD) {
          event_loop.set_control_flow(ControlFlow::WaitUntil(until))
        } else {
          event_loop.set_control_flow(ControlFlow::Wait);
        }
      } else {
        event_loop.set_control_flow(ControlFlow::Wait);
      }
    }
  }

  pub fn set_paused(&mut self, event_loop: &ActiveEventLoop, value: bool) {
    self.paused = value;
    self.update_control_flow(event_loop);
  }

  pub fn set_suspended(&mut self, event_loop: &ActiveEventLoop, value: bool) {
    self.occluded = value;
    self.update_control_flow(event_loop);
  }

  pub fn set_occluded(&mut self, event_loop: &ActiveEventLoop, value: bool) {
    self.suspended = value;
    self.update_control_flow(event_loop);
  }

  pub fn set_waiting_for_window_events(&mut self, event_loop: &ActiveEventLoop, value: bool) {
    self.waiting_for_window_events = value;
    self.update_control_flow(event_loop);
  }
}

impl RenderStatus {
  pub fn new(event_loop: &EventLoop<()>) -> Result<Self, PreWindowInitError> {
    let render = PreWindowInit::new(event_loop)?;
    Ok(RenderStatus::Initialized(render))
  }

  pub fn start(
    self,
    event_loop: &ActiveEventLoop,
    compute_info: &Arc<RwLock<WindowToComputeInfo>>,
  ) -> Result<Self, InitializationError> {
    match self {
      RenderStatus::Initialized(init) => {
        let threads_manager = ThreadsManager::start(init, event_loop, compute_info.clone())?;

        let window_dimensions = threads_manager.window().inner_size();
        {
          let mut write = compute_info.write().unwrap();
          write.initialized = true;
          write.render_dimensions = RenderAreaDimensions::new(window_dimensions);
        }

        Ok(Self::Started(StartedStatus {
          threads_manager,
          paused: START_PAUSED,
          occluded: false,
          suspended: false,
          waiting_for_window_events: false,
        }))
      }
      _ => panic!("Render started multiple times"),
    }
  }

  pub fn unwrap_started(&mut self) -> &mut StartedStatus {
    if let Self::Started(started) = self {
      started
    } else {
      panic!()
    }
  }

  pub fn started(&self) -> bool {
    matches!(self, Self::Started(_))
  }
}

impl App {
  pub fn new(status: RenderStatus) -> Self {
    let window_resize_handler = WindowResizeHandler {
      active: false,
      last_activation_instant: Instant::now(),
      last_activation_size: PhysicalSize {
        width: u32::MAX,
        height: u32::MAX,
      },
    };

    let last_update = Instant::now();
    let time_since_last_fps_print = Duration::ZERO;
    let last_frames_durations = LastFramesDurations::new();

    let frame_i: usize = 0;

    let compute_info = Arc::new(RwLock::new(WindowToComputeInfo::default()));

    Self {
      status,
      window_resize_handler,
      last_update,
      time_since_last_fps_print,
      frame_i,
      last_frames_durations,
      compute_info,
      mouse_position: PhysicalPosition::default(),
      mouse_in_window: false,
    }
  }
}

impl ApplicationHandler for App {
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    if !self.status.started() {
      log::debug!("Starting application");
      take_mut::take(&mut self.status, |status| {
        match status.start(event_loop, &mut self.compute_info) {
          Ok(v) => v,
          Err(err) => {
            log::error!("Failed to start rendering\n{}", err);
            std::process::exit(1);
          }
        }
      });
    } else {
      let status = self.status.unwrap_started();
      log::debug!("Application resumed");
      status.set_suspended(event_loop, false);
    }
  }

  fn device_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    _device_id: winit::event::DeviceId,
    event: winit::event::DeviceEvent,
  ) {
    match event {
      DeviceEvent::MouseMotion { delta } => {
        // try to keep track of the mouse outside the window
        if !self.mouse_in_window {
          let mut write = match self.compute_info.write() {
            Err(err) => {
              log::error!("Window device event: Window input is poisoned {:?}", err);
              event_loop.exit();
              return;
            }
            Ok(v) => v,
          };
          write.mouse_position.x += delta.0;
          write.mouse_position.y += delta.1;
          self.mouse_position.x += delta.0;
          self.mouse_position.y += delta.1;
        }
      }
      _ => {}
    }
  }

  fn window_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    _window_id: winit::window::WindowId,
    event: WindowEvent,
  ) {
    let status = self.status.unwrap_started();
    match event {
      WindowEvent::RedrawRequested => {
        if self.window_resize_handler.active
          && self.window_resize_handler.last_activation_instant.elapsed()
            >= FORCE_WINDOW_RESIZE_DURATION_THRESHOLD
        {
          status.set_waiting_for_window_events(event_loop, false);
          status.threads_manager.window_resized();
          self.window_resize_handler.active = false;
        }

        if !status.should_draw() {
          return;
        }

        let now = Instant::now();
        let time_passed = now - self.last_update;
        self.last_update = now;

        self.last_frames_durations.update_new(time_passed);

        self.time_since_last_fps_print += time_passed;
        if self.time_since_last_fps_print >= PRINT_FPS_EVERY {
          self.time_since_last_fps_print -= PRINT_FPS_EVERY;
          let fps = self.last_frames_durations.get_min_max_average_fps();
          println!("FPS: {:.4} {:.4} {:.4}", fps.min, fps.max, fps.average);
        }

        if self.frame_i <= RENDER_UNTIL_FRAME {
          if DEBUG_PRINT_FRAME_INFO {
            log::debug!("Starting frame {}", self.frame_i);
          }

          if let Err(err) = status.threads_manager.render_next_frame(
            self.frame_i,
            self.last_frames_durations.get_min_max_average_fps(),
          ) {
            match err {
              FrameRenderError::FailedToAcquireSwapchainImage(AcquireNextImageError::OutOfDate) => {
                // window resizes can happen while this function is running and be not detected in time
                // other reasons may include format changes
                log::warn!("Failed to present to swapchain: Swapchain is out of date");
              }
              other => {
                log::error!(
                  "Rendering a frame returned an unrecoverable error\n{}",
                  other
                );
                event_loop.exit();
                return;
              }
            }
          }
        }
        self.frame_i += 1;
        status.threads_manager.window().request_redraw();
      }
      WindowEvent::CloseRequested => {
        event_loop.exit();
      }
      WindowEvent::Occluded(occluded) => {
        status.set_occluded(event_loop, occluded);
      }
      WindowEvent::Resized(new_size) => {
        if WAIT_FOR_MULTIPLE_RESIZE_EVENTS_ENABLED {
          let width_delta = new_size
            .width
            .abs_diff(self.window_resize_handler.last_activation_size.width);
          let height_delta = new_size
            .height
            .abs_diff(self.window_resize_handler.last_activation_size.height);
          let size_delta = width_delta.max(height_delta);

          if size_delta > FORCE_WINDOW_RESIZE_SIZE_THRESHOLD {
            {
              let mut write = self.compute_info.write().unwrap();
              write.render_dimensions = RenderAreaDimensions::new(new_size);
            }
            status.threads_manager.window_resized();

            if self.window_resize_handler.active {
              self.window_resize_handler.active = false;
              status.set_waiting_for_window_events(event_loop, false);
            }
            self.window_resize_handler.last_activation_size = new_size;
            return;
          }

          if !self.window_resize_handler.active {
            status.set_waiting_for_window_events(event_loop, true);

            self.window_resize_handler.active = true;
            self.window_resize_handler.last_activation_instant = Instant::now();
            self.window_resize_handler.last_activation_size = new_size;
          }
        } else {
          {
            let mut write = self.compute_info.write().unwrap();
            write.render_dimensions = RenderAreaDimensions::new(new_size);
          }
          status.threads_manager.window_resized();
        }
        status.threads_manager.window().request_redraw();
      }
      WindowEvent::CursorMoved { position, .. } => {
        let mut write = self.compute_info.write().unwrap();
        write.mouse_position = position;
        self.mouse_position = position;
      }
      WindowEvent::CursorEntered { .. } => {
        self.mouse_in_window = true;
      }
      WindowEvent::CursorLeft { .. } => {
        self.mouse_in_window = false;
      }
      WindowEvent::MouseInput { state, button, .. } => {
        if let MouseButton::Left = button {
          if let Err(err) = self
            .status
            .unwrap_started()
            .threads_manager
            .mouse_click(state, self.mouse_position)
          {
            log::error!("Window event: Failed to forward event to compute {:?}", err);
            event_loop.exit();
          }
        }
      }
      WindowEvent::KeyboardInput { event, .. } => {
        let pressed = event.state.is_pressed();
        let repeating = event.repeat;
        // todo: implement step frame by frame functionality
        if let PhysicalKey::Code(code) = event.physical_key {
          match code {
            // close on escape
            KeyCode::Escape => event_loop.exit(),
            KeyCode::Pause => {
              if pressed {
                if status.paused {
                  log::info!("Unpaused!");
                  status.threads_manager.window().request_redraw();
                } else {
                  log::info!("Paused!");
                }
                status.set_paused(event_loop, !status.paused);
              }
            }
            KeyCode::F2 | KeyCode::F12 => {
              if pressed && !repeating {
                status.threads_manager.screenshot();
              }
            }
            KeyCode::F3 | KeyCode::F10 => {
              if pressed && !repeating {
                // attempt to resize the window to native resolution
                let old_size = status.threads_manager.window().inner_size();
                if old_size.width != RESOLUTION[0] && old_size.height != RESOLUTION[1] {
                  match status
                    .threads_manager
                    .window()
                    .request_inner_size(PhysicalSize {
                      width: RESOLUTION[0],
                      height: RESOLUTION[1],
                    }) {
                    Some(size) => {
                      if size == old_size {
                        println!("Attempted to resize to native resolution, however resizing is currently disallowed by the windowing system.");
                      } else {
                        println!("Attempted to resize to native resolution, however such command may have been ignored by the platform.");
                      }
                    }
                    None => {
                      println!("Successfully resized to native resolution");
                    }
                  }
                }
              }
            }
            _ => {}
          }
        }
      }
      _ => {}
    }
  }

  fn suspended(&mut self, event_loop: &ActiveEventLoop) {
    // should completely pause the application
    // note: not actually implemented for linux/windows
    log::debug!("Application suspended");
    self.status.unwrap_started().set_suspended(event_loop, true);
  }
}

fn main() -> Result<(), EventLoopError> {
  // initialize env_logger with debug if validation layers are enabled, warn otherwise
  #[cfg(feature = "vl")]
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
  #[cfg(not(feature = "vl"))]
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

  let event_loop = EventLoop::new().expect("Failed to initialize event loop");

  // make the event loop run continuously even if there is no new user input
  event_loop.set_control_flow(ControlFlow::Poll);

  let status = match RenderStatus::new(&event_loop) {
    Ok(v) => v,
    Err(err) => {
      log::error!("Failed to initialize Vulkan\n{}", err);
      std::process::exit(1);
    }
  };
  let mut app = App::new(status);

  event_loop.run_app(&mut app)
}
