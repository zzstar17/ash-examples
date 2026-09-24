use std::{f32, time::Duration};

use cgmath::{Euler, Matrix4, Point3, Rad, Vector3};

use crate::{
  keys::{KeyState::Pressed, Keys},
  scene::{
    camera::{Camera, RenderCamera},
    ferris::Ferris,
    obj_3d::Render3dObj,
  },
  RESOLUTION,
};

mod camera;
mod ferris;
mod obj_3d;

pub struct Scene {
  pub ferris: Ferris,
  pub camera: RenderCamera,

  pub ferris_obj: Render3dObj,
  pub niko_obj: Render3dObj,
  pub kakyoin_obj: Render3dObj,
}

pub struct DrawMatrices {
  pub ferris: Matrix4<f32>,
  pub niko: Matrix4<f32>,
  pub kakyoin: Matrix4<f32>,
}

impl Scene {
  pub fn new() -> Self {
    let ferris = Ferris::new([0.2, 0.0], [0.2, 0.15]);

    let aspect_ratio = RESOLUTION[0] as f32 / RESOLUTION[1] as f32;
    let camera = RenderCamera::new(
      Camera::new(1.0, f32::consts::PI / -2.0, 0.0),
      0.8,
      aspect_ratio,
      0.0003,
    );

    let rotation = Euler {
      x: Rad(0.0),
      y: Rad(0.0),
      z: Rad(0.0),
    };
    let ferris_obj = Render3dObj::from_full(
      Point3::new(ferris.pos[0], ferris.pos[1], -3.0),
      rotation,
      Vector3::new(Ferris::WIDTH, Ferris::HEIGHT, 1.0),
    );
    let niko_obj = Render3dObj::from_full(
      Point3::new(8.0, 0.0, -4.0),
      rotation,
      Vector3::new(1.0, 1.0, 1.0),
    );
    let kakyoin_obj = Render3dObj::from_full(
      Point3::new(-8.0, 0.0, -4.0),
      rotation,
      Vector3::new(1.0, 1.0, 1.0),
    );

    Self {
      ferris,
      camera,
      ferris_obj,
      niko_obj,
      kakyoin_obj,
    }
  }

  pub fn update(&mut self, time_since_last_update: Duration, keys: &Keys) {
    self.update_from_keys(keys, time_since_last_update);

    self.ferris.update(time_since_last_update);

    self
      .ferris_obj
      .move_to(Point3::new(self.ferris.pos[0], self.ferris.pos[1], -3.0));
  }

  pub fn get_mvp_matrices(&self) -> DrawMatrices {
    let projection_view = self.camera.projection_view();
    DrawMatrices {
      ferris: projection_view * self.ferris_obj.model(),
      niko: projection_view * self.niko_obj.model(),
      kakyoin: projection_view * self.kakyoin_obj.model(),
    }
  }

  fn update_from_keys(&mut self, keys: &Keys, time_since_last_update: Duration) {
    if keys.a ^ keys.d {
      if keys.a == Pressed {
        self.camera.move_left(&time_since_last_update)
      } else {
        self.camera.move_right(&time_since_last_update)
      }
    }
    if keys.w ^ keys.s {
      if keys.w == Pressed {
        self.camera.move_forward(&time_since_last_update)
      } else {
        self.camera.move_backwards(&time_since_last_update)
      }
    }
    if keys.space ^ keys.l_shift {
      if keys.space == Pressed {
        self.camera.move_up(&time_since_last_update)
      } else {
        self.camera.move_down(&time_since_last_update)
      }
    }
    if keys.q ^ keys.e {
      if keys.q == Pressed {
        self
          .camera
          .rotate(-5000.0 * time_since_last_update.as_secs_f32(), 0.0);
      } else {
        self
          .camera
          .rotate(5000.0 * time_since_last_update.as_secs_f32(), 0.0)
      }
    }
  }
}
