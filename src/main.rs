
mod ferris;
mod render;
mod utility;

use ash::vk;
use ferris::Ferris;
use render::{
  AcquireNextImageError, FrameRenderError, InitializationError, RenderInit, RenderInitError,
  SyncRenderer,
};
use std::{
  ffi::CStr,
  time::{Duration, Instant},
};
use winit::{
  dpi::PhysicalSize,
  event::{Event, WindowEvent},
  event_loop::{ControlFlow, EventLoop, EventLoopWindowTarget},
  keyboard::{KeyCode, PhysicalKey},
};

const APPLICATION_NAME: &CStr = c"Bouncy Ferris";
const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

const WINDOW_TITLE: &str = "Bouncy Ferris";
const INITIAL_WINDOW_WIDTH: u32 = 800;
const INITIAL_WINDOW_HEIGHT: u32 = 800;

const RESOLUTION: [u32; 2] = [800, 800];

const SCREENSHOT_SAVE_FILE: &str = "last_screenshot.png";

const BACKGROUND_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.01, 0.01, 0.01, 1.0],
};
// color exterior the game area
// (that appears if window is resized to a size with ratio different that in RESOLUTION)
const OUT_OF_BOUNDS_AREA_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.0, 0.0, 0.0, 1.0],
};

// see https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPresentModeKHR.html
// FIFO_KHR is required to be supported and functions as vsync
// IMMEDIATE will be chosen over RELAXED_KHR if the latter is not supported
// otherwise, presentation mode will fallback to FIFO_KHR
const PREFERRED_PRESENTATION_METHOD: vk::PresentModeKHR = vk::PresentModeKHR::IMMEDIATE;

// prints current frame 1 / <time since last frame> every x time
const PRINT_FPS_EVERY: Duration = Duration::from_millis(1000);

const START_PAUSED: bool = false; // start application in a paused state

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
  Initialized(RenderInit),
  Started(StartedStatus),
}

struct StartedStatus {
  pub renderer: SyncRenderer,
  pub paused: bool,
  pub occluded: bool,
  pub suspended: bool,
  pub waiting_for_window_events: bool,
}

impl StartedStatus {
  pub fn should_draw(&self) -> bool {
    !self.paused && !self.occluded && !self.suspended && !self.waiting_for_window_events
  }

  // set control flow to poll if frames are ok to draw
  fn update_control_flow(&self, target: &EventLoopWindowTarget<()>) {
    if self.should_draw() {
      target.set_control_flow(ControlFlow::Poll);
    } else if let Some(until) = Instant::now().checked_add(FORCE_WINDOW_RESIZE_DURATION_THRESHOLD) {
      target.set_control_flow(ControlFlow::WaitUntil(until))
    } else {
      target.set_control_flow(ControlFlow::Wait);
    }
  }

  pub fn set_paused(&mut self, target: &EventLoopWindowTarget<()>, value: bool) {
    self.paused = value;
    self.update_control_flow(target);
  }

  pub fn set_suspended(&mut self, target: &EventLoopWindowTarget<()>, value: bool) {
    self.occluded = value;
    self.update_control_flow(target);
  }

  pub fn set_occluded(&mut self, target: &EventLoopWindowTarget<()>, value: bool) {
    self.suspended = value;
    self.update_control_flow(target);
  }

  pub fn set_waiting_for_window_events(&mut self, target: &EventLoopWindowTarget<()>, value: bool) {
    self.waiting_for_window_events = value;
    self.update_control_flow(target);
  }
}

impl RenderStatus {
  pub fn new(event_loop: &EventLoop<()>) -> Result<Self, RenderInitError> {
    let render = RenderInit::new(event_loop)?;
    Ok(RenderStatus::Initialized(render))
  }

  pub fn start(self, event_loop: &EventLoopWindowTarget<()>) -> Result<Self, InitializationError> {
    match self {
      RenderStatus::Initialized(init) => {
        let renderer = init.start(event_loop)?;
        Ok(Self::Started(StartedStatus {
          renderer,
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

fn main_loop(event_loop: EventLoop<()>, mut status: RenderStatus) {
  let mut window_resize_handler = WindowResizeHandler {
    active: false,
    last_activation_instant: Instant::now(),
    last_activation_size: PhysicalSize {
      width: u32::MAX,
      height: u32::MAX,
    },
  };

  let mut ferris = Ferris::new([0.2, 0.0], true, true);

  let mut last_update = Instant::now();
  let mut time_since_last_fps_print = Duration::ZERO;

  let mut frame_i: usize = 0;
  event_loop
    .run(move |event, target| {
      if !status.started() {
        if event == Event::Resumed {
          log::debug!("Starting application");
          take_mut::take(&mut status, |status| match status.start(target) {
            Ok(v) => v,
            Err(err) => {
              log::error!("Failed to start rendering\n{}", err);
              std::process::exit(1);
            }
          });
        }
      } else {
        let status = status.unwrap_started();
        match event {
          Event::Suspended => {
            // should completely pause the application
            log::debug!("Application suspended");
            status.set_suspended(target, true);
          }
          Event::Resumed => {
            log::debug!("Application resumed");
            status.set_suspended(target, false);
          }
          Event::AboutToWait => {
            // winit has two events that notify when a frame needs to be rendered:
            // WindowEvent::RedrawRequested => Useful for applications that don't render often,
            //  triggers only if the system requests for a rerender (for example during window resize)
            //  or if a redraw is requested explicitly (using window.request_redraw()).
            // Event::AboutToWait => Triggered instantly after the previous event once new inputs have
            // been processed. Useful for applications that draw continuously.

            if window_resize_handler.active
              && window_resize_handler.last_activation_instant.elapsed()
                >= FORCE_WINDOW_RESIZE_DURATION_THRESHOLD
            {
              status.set_waiting_for_window_events(target, false);
              status.renderer.window_resized();
              window_resize_handler.active = false;
            }

            if !status.should_draw() {
              return;
            }

            let now = Instant::now();
            let time_passed = now - last_update;
            last_update = now;

            time_since_last_fps_print += time_passed;
            if time_since_last_fps_print >= PRINT_FPS_EVERY {
              time_since_last_fps_print -= PRINT_FPS_EVERY;
              println!("FPS: {}", 1.0 / time_passed.as_secs_f32());
            }

            if frame_i < usize::MAX {
              ferris.update(
                time_passed,
                PhysicalSize {
                  width: RESOLUTION[0],
                  height: RESOLUTION[1],
                },
              );

              // println!("\n\nRENDERING FRAME {}\n", frame_i);
              if let Err(err) = status.renderer.render_next_frame(&ferris) {
                match err {
                  FrameRenderError::FailedToAcquireSwapchainImage(
                    AcquireNextImageError::OutOfDate,
                  ) => {
                    // window resizes can happen while this function is running and be not detected in time
                    // other reasons may include format changes
                    log::warn!("Failed to present to swapchain: Swapchain is out of date");
                  }
                  other => {
                    log::error!(
                      "Rendering a frame returned an unrecoverable error\n{}",
                      other
                    );
                    std::process::exit(1);
                  }
                }
              }
            }
            frame_i += 1;
          }
          Event::WindowEvent { event, .. } => match event {
            WindowEvent::CloseRequested => {
              target.exit();
            }
            WindowEvent::Occluded(occluded) => {
              status.set_occluded(target, occluded);
            }
            WindowEvent::Resized(new_size) => {
              if WAIT_FOR_MULTIPLE_RESIZE_EVENTS_ENABLED {
                let width_delta = new_size
                  .width
                  .abs_diff(window_resize_handler.last_activation_size.width);
                let height_delta = new_size
                  .height
                  .abs_diff(window_resize_handler.last_activation_size.height);
                let size_delta = width_delta.max(height_delta);

                if size_delta > FORCE_WINDOW_RESIZE_SIZE_THRESHOLD {
                  status.renderer.window_resized();

                  if window_resize_handler.active {
                    window_resize_handler.active = false;
                    status.set_waiting_for_window_events(target, false);
                  }
                  window_resize_handler.last_activation_size = new_size;
                  return;
                }

                if !window_resize_handler.active {
                  status.set_waiting_for_window_events(target, true);

                  window_resize_handler.active = true;
                  window_resize_handler.last_activation_instant = Instant::now();
                  window_resize_handler.last_activation_size = new_size;
                }
              } else {
                status.renderer.window_resized();
              }
            }
            WindowEvent::KeyboardInput { event, .. } => {
              let pressed = event.state.is_pressed();
              let repeating = event.repeat;
              // todo: implement step frame by frame functionality
              if let PhysicalKey::Code(code) = event.physical_key {
                match code {
                  // close on escape
                  KeyCode::Escape => target.exit(),
                  KeyCode::Pause => {
                    if pressed {
                      if status.paused {
                        log::info!("Unpaused!");
                      } else {
                        log::info!("Paused!");
                      }
                      status.set_paused(target, !status.paused);
                    }
                  }
                  KeyCode::F2 | KeyCode::F12 => {
                    if pressed && !repeating {
                      status.renderer.screenshot();
                    }
                  }
                  KeyCode::F10 => {
                    if pressed && !repeating {
                      // attempt to resize the window to native resolution
                      let old_size = status.renderer.window().inner_size();
                      if old_size.width != RESOLUTION[0] && old_size.height != RESOLUTION[1] {
                        match status.renderer.window().request_inner_size(PhysicalSize {
                          width: RESOLUTION[0],
                          height: RESOLUTION[1]
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
          },
          _ => (),
        }
      }
    })
    .expect("Failed to run event loop")
}

fn main() {
  env_logger::init();
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
  main_loop(event_loop, status);
}
