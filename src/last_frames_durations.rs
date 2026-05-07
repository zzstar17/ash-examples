use std::time::Duration;

pub struct LastFramesDurations<const N: usize> {
  durations: [Duration; N],
  next_update_i: usize,
}

impl<const N: usize> LastFramesDurations<N> {
  pub fn new() -> Self {
    assert!(N > 0);
    Self {
      durations: [Duration::MAX; N],
      next_update_i: 0,
    }
  }

  pub fn update_new(&mut self, duration: Duration) {
    self.durations[self.next_update_i] = duration;
    self.next_update_i = (self.next_update_i + 1) % N;
  }

  pub fn get_min_max_average_fps(&self) -> (f64, f64, f64) {
    let mut total = 0.0;
    let mut valid_count = 0usize;
    let mut min = f64::MAX;
    let mut max = 0.0;

    for duration in self.durations.iter() {
      if *duration == Duration::MAX {
        continue;
      }
      let secs = duration.as_secs_f64();
      valid_count += 1;

      total += secs;
      if secs < min {
        min = secs;
      }
      if secs > max {
        max = secs;
      }
    }

    if valid_count == 0 {
      return (0.0, 0.0, 0.0);
    }

    let min_fps = 1.0 / max;
    let max_fps = 1.0 / min;
    let average_fps = 1.0 / (total / valid_count as f64);
    (min_fps, max_fps, average_fps)
  }
}
