#[allow(clippy::too_many_arguments)]
mod allocator;
mod command_pools;
mod create_objs;
mod errors;
mod gpu_data;
mod initialization;
mod pipelines;
mod render_pass;
mod renderer;
mod shaders;
mod vertices;

use ash::vk;
use std::ffi::CStr;
use vertices::Vertex;

use crate::renderer::Renderer;

const TARGET_API_VERSION: u32 = vk::API_VERSION_1_3;

static APPLICATION_NAME: &CStr = c"Triangle image";
const APPLICATION_VERSION: u32 = vk::make_api_version(0, 1, 0, 0);

const IMAGE_WIDTH: u32 = 1920;
const IMAGE_HEIGHT: u32 = 1080;

const IMAGE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
const IMAGE_FORMAT_SIZE: u64 = 4;
const IMAGE_MINIMAL_SIZE: u64 = IMAGE_WIDTH as u64 * IMAGE_HEIGHT as u64 * IMAGE_FORMAT_SIZE;

const IMAGE_SAVE_TYPE: image::ColorType = image::ColorType::Rgba8; // should be equivalent

const IMAGE_SAVE_PATH: &str = "image.png";

const BACKGROUND_COLOR: vk::ClearColorValue = vk::ClearColorValue {
  float32: [0.01, 0.01, 0.01, 1.0],
};
static VERTICES: [Vertex; 3] = [
  Vertex {
    pos: [0.7, 0.3],
    color: [1.0, 0.0, 0.0],
  },
  Vertex {
    pos: [-0.4, 0.9],
    color: [0.0, 1.0, 0.0],
  },
  Vertex {
    pos: [-0.9, -0.8],
    color: [0.0, 0.0, 1.0],
  },
];
static INDICES: [u16; 3] = [0, 1, 2];

fn run_app() -> Result<(), String> {
  // initialize env_logger with debug if validation layers are enabled, warn otherwise
  #[cfg(feature = "vl")]
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
  #[cfg(not(feature = "vl"))]
  env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();

  let mut renderer = Renderer::initialize(IMAGE_WIDTH, IMAGE_HEIGHT, IMAGE_MINIMAL_SIZE)
    .map_err(|err| format!("Failed to initialize: {}", err))?;
  unsafe { renderer.record_work() }.map_err(|err| format!("Failed to record work: {}", err))?;

  println!("Submitting work...");
  renderer
    .submit_and_wait()
    .map_err(|err| format!("Failed to submit work: {}", err))?;
  println!("GPU finished!");

  println!("Saving file...");
  let data = unsafe {
    renderer
      .get_resulting_data(IMAGE_MINIMAL_SIZE)
      .map_err(|err| format!("Failed to get resulting data: {}", err))?
  };
  image::save_buffer(
    IMAGE_SAVE_PATH,
    data,
    IMAGE_WIDTH,
    IMAGE_HEIGHT,
    IMAGE_SAVE_TYPE,
  )
  .map_err(|err| format!("Failed to save image: {}", err))?;

  println!("Done!");

  Ok(())
}

fn main() {
  if let Err(err) = run_app() {
    eprintln!("{}", err);
    std::process::exit(1);
  }
}
