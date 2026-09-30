use std::{f32, time::Duration};

use cgmath::{InnerSpace, Matrix4, Point3, Quaternion, Vector3};

use crate::{
  asset_loader::{model_loader::ModelOffset, texture_loader::TextureOffsets, Models},
  keys::{
    KeyState::{Pressed, Released},
    Keys,
  },
  render::GraphicsPushConstants,
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

const UP: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

const CAMERA_MOUSE_SENSITIVITY: f64 = 0.006;
const CAMERA_KEYBOARD_SENSITIVITY: f32 = 2.0;

pub struct Scene {
  pub models: Models,
  pub textures: TextureOffsets,

  pub last_update_projection_view: Matrix4<f32>,

  pub ferris: Ferris,
  pub camera: RenderCamera,

  pub ferris_obj: Render3dObj,
  pub niko_obj: Render3dObj,
  pub kakyoin_obj: Render3dObj,
  pub ferris_borders: [Render3dObj; 4],
  pub crosshair: [Render3dObj; 3],

  pub niko_text: Render3dObj,
  pub kakyoin_text: Render3dObj,
}

// axis should be normalized
fn quaternion_from_angle(angle: f32, axis_of_rotation: Vector3<f32>) -> Quaternion<f32> {
  Quaternion {
    s: (angle / 2.0).cos(),
    v: (angle / 2.0).sin() * axis_of_rotation,
  }
}

impl Scene {
  pub fn new(models: Models, textures: TextureOffsets) -> Self {
    let ferris = Ferris::new([0.2, 0.0], [0.2, 0.15]);

    let aspect_ratio = RESOLUTION[0] as f32 / RESOLUTION[1] as f32;
    let camera = RenderCamera::new(
      Camera::new(1.0, f32::consts::PI / -2.0, 0.0),
      0.8,
      aspect_ratio,
      3.0,
    );

    let angle: f32 = 0.0;

    let ferris_obj = Render3dObj::from_full(
      Point3::new(ferris.pos[0], ferris.pos[1], -3.0),
      Quaternion::new(1.0, 0.0, 0.0, 0.0),
      Vector3::new(Ferris::WIDTH, Ferris::HEIGHT, 1.0),
    );
    let ferris_borders = [
      Render3dObj::from_full(
        Point3::new(0.5, 1.0 + 0.066, -2.95),
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vector3::new(0.59, 0.03, 1.0),
      ),
      Render3dObj::from_full(
        Point3::new(0.5, 0.0 - 0.066, -2.95),
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vector3::new(0.59, 0.03, 1.0),
      ),
      Render3dObj::from_full(
        Point3::new(0.0 - 0.066, 0.5, -2.95),
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vector3::new(0.03, 0.59, 1.0),
      ),
      Render3dObj::from_full(
        Point3::new(1.0 + 0.066, 0.5, -2.95),
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vector3::new(0.03, 0.59, 1.0),
      ),
    ];
    let crosshair = [
      Render3dObj::from_full(
        Point3::new(0.5, 0.0, 0.0),
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vector3::new(0.5, 0.05, 0.05),
      ),
      Render3dObj::from_full(
        Point3::new(0.0, 0.5, 0.0),
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vector3::new(0.05, 0.5, 0.05),
      ),
      Render3dObj::from_full(
        Point3::new(0.0, 0.0, 0.5),
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vector3::new(0.05, 0.05, 0.5),
      ),
    ];
    let niko_obj = Render3dObj::from_full(
      Point3::new(16.0, 0.0, -4.0),
      quaternion_from_angle(angle, Vector3::new(0.2, 1.0, 0.0).normalize()),
      Vector3::new(1.0, 1.0, 1.0),
    );
    let kakyoin_obj = Render3dObj::from_full(
      Point3::new(-16.0, 0.0, -4.0),
      quaternion_from_angle(angle, Vector3::new(0.0, 1.0, 0.0)),
      Vector3::new(1.0, 1.0, 1.0),
    );

    let mut niko_text = niko_obj.clone();
    niko_text.move_y(-3.5);
    let mut kakyoin_text = kakyoin_obj.clone();
    kakyoin_text.move_y(-1.5);

    Self {
      models,
      textures,
      last_update_projection_view: Matrix4::from_scale(1.0),

      ferris,
      camera,
      ferris_obj,
      ferris_borders,
      niko_obj,
      kakyoin_obj,
      crosshair,

      niko_text,
      kakyoin_text,
    }
  }

  pub fn update(
    &mut self,
    time_since_last_update: Duration,
    keys: &Keys,
    camera_mov: Option<[f64; 2]>,
  ) {
    if let Some(mov) = camera_mov {
      // todo: winit seems to be worse at keeping track of physical device mouse movements the higher the framerate
      // (at least on wayland)
      // jank solution
      let delta_x = mov[0]
        * (time_since_last_update.as_nanos() as f64 / 1000000000.0).powf(0.1)
        * CAMERA_MOUSE_SENSITIVITY;
      let delta_y = mov[1]
        * (time_since_last_update.as_nanos() as f64 / 1000000000.0).powf(0.1)
        * CAMERA_MOUSE_SENSITIVITY;
      self.camera.rotate(delta_x as f32, delta_y as f32);
    }
    self.update_from_keys(keys, time_since_last_update);
    self.last_update_projection_view = self.camera.projection_view();

    let crosshair_position = self.camera.position() + self.camera.front() * 50.0;
    self.crosshair[0].set_position(crosshair_position + Vector3::new(0.5, 0.0, 0.0));
    self.crosshair[1].set_position(crosshair_position + Vector3::new(0.0, 0.5, 0.0));
    self.crosshair[2].set_position(crosshair_position + Vector3::new(0.0, 0.0, 0.5));

    self.ferris.update(time_since_last_update);

    let angle = 0.04 * time_since_last_update.as_secs_f32();
    // rotate around y axis
    let rotation = quaternion_from_angle(angle, Vector3::new(0.0, 1.0, 0.0));

    self.kakyoin_obj.rotate(rotation);
    self.niko_obj.rotate(rotation);

    // make text gradually rotate towards the camera
    let rotate_amount = 2.0 * time_since_last_update.as_secs_f32();
    self
      .niko_text
      .linearly_vertically_rotate_to_point(self.camera.position(), rotate_amount);
    self
      .kakyoin_text
      .linearly_vertically_rotate_to_point(self.camera.position(), rotate_amount);

    self
      .ferris_obj
      .set_position(Point3::new(self.ferris.pos[0], self.ferris.pos[1], -3.0));
  }

  pub fn get_ferris_data(&self) -> (GraphicsPushConstants, ModelOffset) {
    let push_constants = GraphicsPushConstants {
      matrix: self.last_update_projection_view * self.ferris_obj.model(),
      tex_offset: self.textures.ferris.offset,
      tex_size: self.textures.ferris.size,
    };

    (push_constants, self.models.quad)
  }

  pub fn get_ferris_borders_data(&self) -> [(GraphicsPushConstants, ModelOffset); 4] {
    let fun = |obj: &Render3dObj| {
      let push_constants = GraphicsPushConstants {
        matrix: self.last_update_projection_view * obj.model(),
        tex_offset: self.textures.black.offset,
        tex_size: self.textures.black.size,
      };

      (push_constants, self.models.quad)
    };

    [
      fun(&self.ferris_borders[0]),
      fun(&self.ferris_borders[1]),
      fun(&self.ferris_borders[2]),
      fun(&self.ferris_borders[3]),
    ]
  }

  pub fn get_crosshair_data(&self) -> [(GraphicsPushConstants, ModelOffset); 3] {
    let pc_x = GraphicsPushConstants {
      matrix: self.last_update_projection_view * self.crosshair[0].model(),
      tex_offset: self.textures.red.offset,
      tex_size: self.textures.red.size,
    };
    let pc_y = GraphicsPushConstants {
      matrix: self.last_update_projection_view * self.crosshair[1].model(),
      tex_offset: self.textures.green.offset,
      tex_size: self.textures.green.size,
    };
    let pc_z = GraphicsPushConstants {
      matrix: self.last_update_projection_view * self.crosshair[2].model(),
      tex_offset: self.textures.blue.offset,
      tex_size: self.textures.blue.size,
    };

    [
      (pc_x, self.models.cube),
      (pc_y, self.models.cube),
      (pc_z, self.models.cube),
    ]
  }

  pub fn get_niko_data(&self) -> (GraphicsPushConstants, ModelOffset) {
    let push_constants = GraphicsPushConstants {
      matrix: self.last_update_projection_view * self.niko_obj.model(),
      tex_offset: self.textures.niko.offset,
      tex_size: self.textures.niko.size,
    };

    (push_constants, self.models.niko)
  }

  pub fn get_kakyoin(&self) -> (GraphicsPushConstants, ModelOffset) {
    let push_constants = GraphicsPushConstants {
      matrix: self.last_update_projection_view * self.kakyoin_obj.model(),
      tex_offset: self.textures.kakyoin.offset,
      tex_size: self.textures.kakyoin.size,
    };

    (push_constants, self.models.kakyoin)
  }

  fn update_from_keys(&mut self, keys: &Keys, time_since_last_update: Duration) {
    if keys.l_ctrl == Pressed {
      let speed = self.camera.speed_mut();
      *speed = 7.0;
    }
    if keys.l_ctrl == Released {
      let speed = self.camera.speed_mut();
      *speed = 1.0;
    }
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
        self.camera.rotate(
          -CAMERA_KEYBOARD_SENSITIVITY * time_since_last_update.as_secs_f32(),
          0.0,
        );
      } else {
        self.camera.rotate(
          CAMERA_KEYBOARD_SENSITIVITY * time_since_last_update.as_secs_f32(),
          0.0,
        )
      }
    }
  }
}
