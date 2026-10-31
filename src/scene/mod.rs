use std::{
  f32::{self, consts::PI},
  time::Duration,
};

use cgmath::{InnerSpace, Matrix4, Point3, Quaternion, SquareMatrix, Vector3};

use crate::{
  asset_loader::{
    model_loader::{AssetLocationData, ImageSize},
    Mesh,
  },
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
mod niko;
mod obj_3d;
mod scene_node;

use niko::Niko;

pub use scene_node::SceneNode;

const UP: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

const CAMERA_MOUSE_SENSITIVITY: f64 = 0.006;
const CAMERA_KEYBOARD_SENSITIVITY: f32 = 2.0;

pub struct Scene {
  pub meshes: Vec<Mesh>,
  pub textures: Vec<ImageSize>,
  pub quad_mesh_index: usize,
  pub cube_mesh_index: usize,
  // pub textures: TextureOffsets,
  pub last_update_projection_view: Matrix4<f32>,

  // pub ferris: Ferris,
  pub camera: RenderCamera,

  child_nodes: Vec<SceneNode>,
  niko: Niko,
  // pub ferris_obj: Render3dObj,
  // pub niko_obj: Render3dObj,
  // pub niko_light_obj: Render3dObj,
  pub kakyoin: SceneNode,
  pub floor: SceneNode,
  // pub ferris_borders: [Render3dObj; 4],
  pub crosshair: [Render3dObj; 3],

  pub niko_text: Render3dObj,
  pub kakyoin_text: Render3dObj,
  pub solid_draw_meshes: Vec<(GraphicsPushConstants, Mesh)>,
  pub semi_transparent_draw_meshes: Vec<(GraphicsPushConstants, Mesh)>,
}

// axis should be normalized
fn quaternion_from_angle(angle: f32, axis_of_rotation: Vector3<f32>) -> Quaternion<f32> {
  Quaternion {
    s: (angle / 2.0).cos(),
    v: (angle / 2.0).sin() * axis_of_rotation,
  }
}

impl Scene {
  pub fn new(asset_locations: AssetLocationData) -> Self {
    println!("meshes:\n{:#?}", asset_locations.meshes);

    let ferris = Ferris::new([0.2, 0.0], [0.2, 0.15]);

    let aspect_ratio = RESOLUTION[0] as f32 / RESOLUTION[1] as f32;
    let camera = RenderCamera::new(
      Camera::new(
        Point3 {
          x: 0.0,
          y: -1.0,
          z: 0.0,
        },
        1.0,
        f32::consts::PI / 2.0,
        0.0,
      ),
      0.8,
      aspect_ratio,
    );

    // gltf positions imported from blender are rotated with z being up
    let world_transform = Render3dObj::from_full(
      Point3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
      },
      quaternion_from_angle(
        PI / 2.0,
        Vector3 {
          x: 1.0,
          y: 0.0,
          z: 0.0,
        },
      ),
      Vector3 {
        x: 0.1,
        y: 0.1,
        z: 0.1,
      },
    )
    .model();

    let mut child_nodes = Vec::new();
    let scene = asset_locations.gltf.scenes().next().unwrap();
    let scene_collection = scene.nodes().next().unwrap();

    let mut niko = Niko::new(&asset_locations, &scene_collection, &mut child_nodes);
    niko.obj = Render3dObj::from_model_matrix(world_transform * niko.obj.model());

    let mut kakyoin = SceneNode::from_gltf(
      &asset_locations,
      scene_collection
        .children()
        .find(|node| node.name().unwrap() == "Kakyoin")
        .unwrap(),
      true,
      &mut child_nodes,
    );
    kakyoin.apply_transform_matrix(&world_transform);

    let mut floor = SceneNode::from_gltf(
      &asset_locations,
      scene_collection
        .children()
        .find(|node| node.name().unwrap() == "Floor")
        .unwrap(),
      true,
      &mut child_nodes,
    );
    floor.apply_transform_matrix(&world_transform);

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

    // let mut niko_text = niko.obj;
    let text_size = 0.2;
    let mut niko_text = Render3dObj::from_full(
      niko.obj.position(),
      niko.obj.rotation,
      Vector3 {
        x: text_size,
        y: text_size,
        z: text_size,
      },
    );
    niko_text.move_y(-1.3);
    let kakyoin_obj = kakyoin.obj.unwrap_obj();
    let mut kakyoin_text = Render3dObj::from_full(
      kakyoin_obj.position(),
      kakyoin_obj.rotation,
      Vector3 {
        x: text_size,
        y: text_size,
        z: text_size,
      },
    );
    kakyoin_text.move_y(-1.9);

    Self {
      meshes: asset_locations.meshes,
      textures: asset_locations.image_sizes,
      quad_mesh_index: asset_locations.quad_mesh_index,
      cube_mesh_index: asset_locations.cube_mesh_index,
      last_update_projection_view: Matrix4::from_scale(1.0),

      camera,

      child_nodes,
      niko,
      kakyoin,
      floor,
      crosshair,

      niko_text,
      kakyoin_text,
      solid_draw_meshes: Vec::new(),
      semi_transparent_draw_meshes: Vec::new(),
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

    // self.ferris.update(time_since_last_update);

    let angle = 0.04 * time_since_last_update.as_secs_f32();
    // rotate around y axis
    let rotation = quaternion_from_angle(angle, Vector3::new(0.0, 1.0, 0.0));

    self.kakyoin.obj.unwrap_obj_mut().rotate(rotation);
    self.niko.obj.rotate(rotation);

    // make text gradually rotate towards the camera
    let rotate_amount = 2.0 * time_since_last_update.as_secs_f32();
    self
      .niko_text
      .linearly_vertically_rotate_to_point(self.camera.position(), rotate_amount);
    self
      .kakyoin_text
      .linearly_vertically_rotate_to_point(self.camera.position(), rotate_amount);

    // self
    //   .ferris_obj
    //   .set_position(Point3::new(self.ferris.pos[0], self.ferris.pos[1], -3.0));

    self.solid_draw_meshes.clear();
    self.semi_transparent_draw_meshes.clear();
    self.niko.add_draw_meshes(
      &self.child_nodes,
      &self.last_update_projection_view,
      &mut self.solid_draw_meshes,
      &mut self.semi_transparent_draw_meshes,
    );
    self.kakyoin.add_draw_meshes(
      &self.child_nodes,
      self.last_update_projection_view,
      &mut self.solid_draw_meshes,
      &mut self.semi_transparent_draw_meshes,
    );
    self.floor.add_draw_meshes(
      &self.child_nodes,
      self.last_update_projection_view,
      &mut self.solid_draw_meshes,
      &mut self.semi_transparent_draw_meshes,
    );
  }

  // pub fn get_ferris_data(&self) -> (GraphicsPushConstants, ModelOffset) {
  //   let push_constants = GraphicsPushConstants {
  //     matrix: self.last_update_projection_view * self.ferris_obj.model(),
  //     tex_offset: self.textures.ferris.offset,
  //     tex_size: self.textures.ferris.size,
  //   };

  //   (push_constants, self.models.quad)
  // }

  // pub fn get_ferris_borders_data(&self) -> [(GraphicsPushConstants, ModelOffset); 4] {
  //   let fun = |obj: &Render3dObj| {
  //     let push_constants = GraphicsPushConstants {
  //       matrix: self.last_update_projection_view * obj.model(),
  //       tex_offset: self.textures.black.offset,
  //       tex_size: self.textures.black.size,
  //     };

  //     (push_constants, self.models.quad)
  //   };

  //   [
  //     fun(&self.ferris_borders[0]),
  //     fun(&self.ferris_borders[1]),
  //     fun(&self.ferris_borders[2]),
  //     fun(&self.ferris_borders[3]),
  //   ]
  // }

  // pub fn get_crosshair_data(&self) -> [(GraphicsPushConstants, Mesh); 3] {
  //   let pc_x = GraphicsPushConstants {
  //     matrix: self.last_update_projection_view * self.crosshair[0].model(),
  //     tex_offset: self.textures.red.offset,
  //     tex_size: self.textures.red.size,
  //   };
  //   let pc_y = GraphicsPushConstants {
  //     matrix: self.last_update_projection_view * self.crosshair[1].model(),
  //     tex_offset: self.textures.green.offset,
  //     tex_size: self.textures.green.size,
  //   };
  //   let pc_z = GraphicsPushConstants {
  //     matrix: self.last_update_projection_view * self.crosshair[2].model(),
  //     tex_offset: self.textures.blue.offset,
  //     tex_size: self.textures.blue.size,
  //   };

  //   [
  //     (pc_x, self.models.cube),
  //     (pc_y, self.models.cube),
  //     (pc_z, self.models.cube),
  //   ]
  // }

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
