use std::time::Duration;

use winit::dpi::PhysicalSize;

use crate::render::RenderPosition;

pub struct Ferris {
  pub position: [f32; 2],
  pub going_right: bool,
  pub going_down: bool,
}

impl Ferris {
  const TEXTURE_DIMENSIONS: [f32; 2] = [428.0, 283.0]; // saved texture dimensions
  const TEXTURE_TO_SIZE_RATIO: f32 = 0.3;
  // size in pixels
  pub const WIDTH: u32 = (Self::TEXTURE_DIMENSIONS[0] * Self::TEXTURE_TO_SIZE_RATIO) as u32;
  pub const HEIGHT: u32 = (Self::TEXTURE_DIMENSIONS[1] * Self::TEXTURE_TO_SIZE_RATIO) as u32;

  const SPEED_X: f32 = 80.0; // speed in pixels per second
  const SPEED_Y: f32 = 80.0;

  pub fn new(position: [f32; 2], going_right: bool, going_down: bool) -> Self {
    Self {
      position,
      going_right,
      going_down,
    }
  }

  pub fn update(&mut self, time_since_last_update: Duration, window_size: PhysicalSize<u32>) {
    let secs_f32 = time_since_last_update.as_secs_f32();
    let delta_pos_x = secs_f32 * Self::SPEED_X;
    let delta_pos_y = secs_f32 * Self::SPEED_Y;
    let window_width = (window_size.width - Self::WIDTH) as f32;
    let window_height = (window_size.height - Self::HEIGHT) as f32;

    let (new_x, x_dir_changed) = Self::calculate_position(
      self.position[0],
      delta_pos_x,
      window_width,
      self.going_right,
    );
    self.position[0] = new_x.min(window_width);
    if x_dir_changed {
      self.going_right = !self.going_right;
    }

    let (new_y, y_dir_changed) = Self::calculate_position(
      self.position[1],
      delta_pos_y,
      window_height,
      self.going_down,
    );
    self.position[1] = new_y.min(window_height);
    if y_dir_changed {
      self.going_down = !self.going_down;
    }
  }

  // calculates position after some time passed
  // returns new position and a boolean that indicates if direction changed
  fn calculate_position(
    initial: f32,
    mut delta: f32,
    size: f32, // size of the window subtracting sprite size
    positive_direction: bool,
  ) -> (f32, bool) {
    delta %= size * 2.0; // remove double bounces
    let mut new = initial;
    if positive_direction {
      new += delta;
      let overflow = new - size;
      if overflow > 0.0 {
        new -= overflow;
        return (new, true);
      }
    } else {
      new -= delta;
      if new < 0.0 {
        new = -new;
        return (new, true);
      }
    }
    (new, false)
  }

  pub fn get_render_position(&self, window_size: PhysicalSize<u32>) -> RenderPosition {
    let window_size_float = PhysicalSize {
      width: window_size.width as f32,
      height: window_size.height as f32,
    };

    let normal_pos = [
      ((self.position[0] * 2.0) - window_size_float.width) / window_size_float.width,
      ((self.position[1] * 2.0) - window_size_float.height) / window_size_float.height,
    ];
    let ratio = [
      Ferris::WIDTH as f32 / window_size_float.width,
      Ferris::HEIGHT as f32 / window_size_float.height,
    ];
    RenderPosition::new(normal_pos, ratio)
  }
}
