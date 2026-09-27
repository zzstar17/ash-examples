use std::{f32, time::Duration};

use ash::vk;
use cgmath::{InnerSpace, Matrix4, Point3, Quaternion, Vector3};
use vkobjects::utility;

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

pub struct Scene {
  pub drawing_data: Vec<GraphicsPushConstants>,
  pub models: Models,
  pub textures: TextureOffsets,

  pub ferris: Ferris,
  pub camera: RenderCamera,

  pub ferris_obj: Render3dObj,
  pub niko_obj: Render3dObj,
  pub kakyoin_obj: Render3dObj,
  pub ferris_borders: [Render3dObj; 4],
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
      0.0003,
    );

    let angle: f32 = f32::consts::PI / 2.0;

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

    let drawing_data = vec![GraphicsPushConstants::default(); 7];

    Self {
      drawing_data,
      models,
      textures,
      ferris,
      camera,
      ferris_obj,
      ferris_borders,
      niko_obj,
      kakyoin_obj,
    }
  }

  pub fn update(&mut self, time_since_last_update: Duration, keys: &Keys) {
    self.update_from_keys(keys, time_since_last_update);

    self.ferris.update(time_since_last_update);

    let angle = 0.02 * time_since_last_update.as_secs_f32();
    // rotate around y axis
    let rotation = quaternion_from_angle(angle, Vector3::new(0.0, 1.0, 0.0));

    self.kakyoin_obj.rotate(rotation);
    self.niko_obj.rotate(rotation);

    self
      .ferris_obj
      .move_to(Point3::new(self.ferris.pos[0], self.ferris.pos[1], -3.0));
  }

  fn get_ferris_data(&self, projection_view: Matrix4<f32>) -> (GraphicsPushConstants, ModelOffset) {
    let push_constants = GraphicsPushConstants {
      matrix: projection_view * self.ferris_obj.model(),
      tex_offset: self.textures.ferris.offset,
      tex_size: self.textures.ferris.size,
    };

    (push_constants, self.models.quad)
  }

  fn get_ferris_borders_data(
    &self,
    projection_view: Matrix4<f32>,
  ) -> [(GraphicsPushConstants, ModelOffset); 4] {
    let fun = |obj: &Render3dObj| {
      let push_constants = GraphicsPushConstants {
        matrix: projection_view * obj.model(),
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

  fn get_niko_data(&self, projection_view: Matrix4<f32>) -> (GraphicsPushConstants, ModelOffset) {
    let push_constants = GraphicsPushConstants {
      matrix: projection_view * self.niko_obj.model(),
      tex_offset: self.textures.niko.offset,
      tex_size: self.textures.niko.size,
    };

    (push_constants, self.models.niko)
  }

  fn get_kakyoin(&self, projection_view: Matrix4<f32>) -> (GraphicsPushConstants, ModelOffset) {
    let push_constants = GraphicsPushConstants {
      matrix: projection_view * self.kakyoin_obj.model(),
      tex_offset: self.textures.kakyoin.offset,
      tex_size: self.textures.kakyoin.size,
    };

    (push_constants, self.models.kakyoin)
  }

  pub unsafe fn record_draw_calls(
    &self,
    device: &ash::Device,
    cb: vk::CommandBuffer,
    pipeline_layout: vk::PipelineLayout,
  ) {
    let record_cmds = |(push_constants, model_offsets): (GraphicsPushConstants, ModelOffset)| {
      device.cmd_push_constants(
        cb,
        pipeline_layout,
        vk::ShaderStageFlags::VERTEX,
        0,
        utility::any_as_u8_slice(&push_constants),
      );
      device.cmd_draw_indexed(
        cb,
        model_offsets.indices_len as u32,
        1,
        model_offsets.indices_offset as u32,
        model_offsets.vertices_offset as i32,
        0,
      );
    };

    let projection_view = self.camera.projection_view();

    record_cmds(self.get_ferris_data(projection_view));
    for data in self.get_ferris_borders_data(projection_view) {
      record_cmds(data);
    }
    record_cmds(self.get_niko_data(projection_view));
    record_cmds(self.get_kakyoin(projection_view));
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
