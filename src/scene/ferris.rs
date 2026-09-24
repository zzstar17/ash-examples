use std::time::Duration;

use crate::FERRIS_TEXTURE_SIZE;

pub struct Ferris {
  // 0 to 1
  pub pos: [f32; 2],
  // speed units per second
  pub vel: [f32; 2],
}

impl Ferris {
  // ferris bounces around a 1*1 unit square
  const SIZE: f32 = 0.1;

  const RATIO: f32 = FERRIS_TEXTURE_SIZE[0] / FERRIS_TEXTURE_SIZE[1];

  pub const WIDTH: f32 = Self::SIZE;
  pub const HEIGHT: f32 = Self::SIZE / Self::RATIO;

  pub fn new(pos: [f32; 2], vel: [f32; 2]) -> Self {
    Self { pos, vel }
  }

  pub fn update(&mut self, time_since_last_update: Duration) {
    let secs_f32 = time_since_last_update.as_secs_f32();

    let delta_pos_x = secs_f32 * self.vel[0];
    let delta_pos_y = secs_f32 * self.vel[1];

    let (new_x, x_dir_changed) =
      Self::calculate_position(self.pos[0], delta_pos_x, Self::WIDTH, 1.0);
    if x_dir_changed {
      self.vel[0] = -self.vel[0];
    }

    let (new_y, y_dir_changed) =
      Self::calculate_position(self.pos[1], delta_pos_y, Self::HEIGHT, 1.0);
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
    bounce_area_size: f32,
  ) -> (f32, bool) {
    let traversable_length = bounce_area_size - sprite_size;

    // subtract double bounces
    let double_size = traversable_length * 2.0;
    if delta > double_size {
      // how many times traversable_length * 2 fits in delta
      let delta_times = (delta / double_size) as usize;
      delta -= delta_times as f32 * double_size;
    }

    let half_sprite_size = sprite_size / 2.0;
    let upper_limit = bounce_area_size - half_sprite_size;
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
}
