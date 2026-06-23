use std::time::Duration;

use winit::dpi::PhysicalSize;

use crate::render::RenderPosition;

pub struct Ferris {
  pub pos: [f32; 2],
  // speed in pixels per second
  pub vel: [f32; 2],
}

impl Ferris {
  const TEXTURE_DIMENSIONS: [f32; 2] = [120.0, 80.0]; // saved texture dimensions
  const TEXTURE_TO_SIZE_RATIO: f32 = 1.0;
  // size in pixels
  pub const WIDTH: u32 = (Self::TEXTURE_DIMENSIONS[0] * Self::TEXTURE_TO_SIZE_RATIO) as u32;
  pub const HEIGHT: u32 = (Self::TEXTURE_DIMENSIONS[1] * Self::TEXTURE_TO_SIZE_RATIO) as u32;

  pub fn new(pos: [f32; 2], vel: [f32; 2]) -> Self {
    Self { pos, vel }
  }

  pub fn update(&mut self, time_since_last_update: Duration, render_size: PhysicalSize<u32>) {
    let secs_f32 = time_since_last_update.as_secs_f32();
    let delta_pos_x = secs_f32 * self.vel[0];
    let delta_pos_y = secs_f32 * self.vel[1];

    let (new_x, x_dir_changed) = Self::calculate_position(
      self.pos[0],
      delta_pos_x,
      Self::TEXTURE_DIMENSIONS[0],
      render_size.width as f32,
    );
    if x_dir_changed {
      self.vel[0] = -self.vel[0];
    }

    let (new_y, y_dir_changed) = Self::calculate_position(
      self.pos[1],
      delta_pos_y,
      Self::TEXTURE_DIMENSIONS[1],
      render_size.height as f32,
    );
    if y_dir_changed {
      self.vel[1] = -self.vel[1];
    }

    self.pos = [new_x, new_y];
  }

  // calculates position after some time passed
  // returns new position and a boolean that indicates if direction changed
  fn calculate_position(
    pos: f32,
    mut delta: f32,
    sprite_size: f32,
    render_size: f32,
  ) -> (f32, bool) {
    let traversable_length = render_size - sprite_size;

    // subtract double bounces
    let double_size = traversable_length * 2.0;
    if delta > double_size {
      // how many times traversable_length * 2 fits in delta
      let delta_times = (delta / double_size) as usize;
      delta -= delta_times as f32 * double_size;
    }

    let half_sprite_size = sprite_size / 2.0;
    let upper_limit = render_size - half_sprite_size;
    let lower_limit = half_sprite_size;

    let mut new_pos = pos + delta;
    let mut direction_changed = false;
    if new_pos > upper_limit {
      let overflow = new_pos - upper_limit;

      new_pos -= overflow * 2.0;
      direction_changed = true;
    } else if new_pos < lower_limit {
      let overflow = new_pos - lower_limit;

      new_pos -= overflow * 2.0;
      direction_changed = true;
    }

    (new_pos, direction_changed)
  }

  pub fn get_render_position(&self, render_size: PhysicalSize<u32>) -> RenderPosition {
    let render_dimensions_f = PhysicalSize {
      width: render_size.width as f32,
      height: render_size.height as f32,
    };

    let normal_pos = [
      self.pos[0] / render_dimensions_f.width,
      self.pos[1] / render_dimensions_f.height,
    ];
    let ratio = [
      Ferris::WIDTH as f32 / render_dimensions_f.width,
      Ferris::HEIGHT as f32 / render_dimensions_f.height,
    ];
    RenderPosition::new(normal_pos, ratio)
  }
}
