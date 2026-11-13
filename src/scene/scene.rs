use std::{
  f32::{self, consts::PI},
  time::Duration,
};

use cgmath::{Matrix4, Point3, Quaternion, Vector3};

use crate::{
  asset_loader::{model_loader::AssetLocationData, Mesh},
  keys::{
    KeyState::{Pressed, Released},
    Keys,
  },
  render::GraphicsPushConstants,
  scene::{
    camera::{Camera, RenderCamera},
    niko::Niko,
    quaternion_from_angle,
    scene_node::ObjEnum,
    Render3dObj, SceneNode, CAMERA_KEYBOARD_SENSITIVITY, CAMERA_MOUSE_SENSITIVITY,
  },
  RESOLUTION,
};

pub struct Scene {
  pub meshes: Vec<Mesh>,
  pub quad_mesh_index: usize,
  pub cube_mesh_index: usize,
  pub last_update_projection_view: Matrix4<f32>,

  pub camera: RenderCamera,

  pub child_nodes: Vec<SceneNode>,
  pub niko: Niko,
  pub kakyoin: SceneNode,
  pub floor: SceneNode,
  pub crosshair: [(Render3dObj, [f32; 4]); 3],
  pub test_cube: SceneNode,

  pub niko_text: Render3dObj,
  pub kakyoin_text: Render3dObj,

  pub solid_draw_meshes: Vec<(GraphicsPushConstants, Mesh)>,
  pub semi_transparent_draw_meshes: Vec<(GraphicsPushConstants, Mesh)>,
  pub drawing_bbs: bool,
}

impl Scene {
  pub fn new(asset_locations: AssetLocationData) -> Self {
    let aspect_ratio = RESOLUTION[0] as f32 / RESOLUTION[1] as f32;
    let camera = RenderCamera::new(
      Camera::new(
        Point3 {
          x: 0.0,
          y: -1.0,
          z: -1.0,
        },
        1.0,
        f32::consts::PI / 2.0,
        0.0,
      ),
      0.8,
      aspect_ratio,
    );

    // gltf positions imported from blender are rotated with z being up
    let gltf_transform = *Render3dObj::from_full(
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
    let gltf_transform_obj = ObjEnum::Matrix(gltf_transform);

    let mut child_nodes = Vec::new();
    let scene = asset_locations.gltf.scenes().next().unwrap();
    let scene_collection = scene.nodes().next().unwrap();

    let niko = Niko::new(
      &asset_locations,
      &scene_collection,
      &gltf_transform,
      &mut child_nodes,
    );

    let kakyoin = SceneNode::from_gltf(
      &asset_locations,
      scene_collection
        .children()
        .find(|node| node.name().unwrap() == "Kakyoin")
        .unwrap(),
      None,
      Some(gltf_transform_obj),
      true,
      &mut child_nodes,
    );

    let floor = SceneNode::from_gltf(
      &asset_locations,
      scene_collection
        .children()
        .find(|node| node.name().unwrap() == "Floor")
        .unwrap(),
      None,
      Some(gltf_transform_obj),
      true,
      &mut child_nodes,
    );

    let mut mesh = asset_locations.meshes[asset_locations.cube_mesh_index];
    mesh.image_index = floor.unwrap_leaf().mesh.image_index;
    let test_cube = SceneNode::from_obj(
      Render3dObj::from_full(
        Point3 {
          x: 0.0,
          y: -0.5,
          z: -5.0,
        },
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vector3 {
          x: 0.5,
          y: 0.5,
          z: 0.5,
        },
      ),
      mesh,
    );

    let crosshair: [(Render3dObj, [f32; 4]); 3] = [
      (
        Render3dObj::from_full(
          Point3::new(0.5, 0.0, 0.0),
          Quaternion::new(1.0, 0.0, 0.0, 0.0),
          Vector3::new(0.5, 0.05, 0.05),
        ),
        [1.0, 0.0, 0.0, 1.0],
      ),
      (
        Render3dObj::from_full(
          Point3::new(0.0, 0.5, 0.0),
          Quaternion::new(1.0, 0.0, 0.0, 0.0),
          Vector3::new(0.05, 0.5, 0.05),
        ),
        [0.0, 1.0, 0.0, 1.0],
      ),
      (
        Render3dObj::from_full(
          Point3::new(0.0, 0.0, 0.5),
          Quaternion::new(1.0, 0.0, 0.0, 0.0),
          Vector3::new(0.05, 0.05, 0.5),
        ),
        [0.0, 0.0, 1.0, 1.0],
      ),
    ];

    // let mut niko_text = niko.obj;
    let text_size = 0.2;
    let mut niko_text = Render3dObj::from_full(
      niko.node.obj().unwrap_obj().position(),
      niko.node.obj().unwrap_obj().rotation,
      Vector3 {
        x: text_size,
        y: text_size,
        z: text_size,
      },
    );
    niko_text.move_y(-1.3);
    let kakyoin_obj = kakyoin.obj().unwrap_obj();
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
      quad_mesh_index: asset_locations.quad_mesh_index,
      cube_mesh_index: asset_locations.cube_mesh_index,
      last_update_projection_view: Matrix4::from_scale(1.0),

      camera,

      child_nodes,
      niko,
      kakyoin,
      floor,
      test_cube,
      crosshair,

      niko_text,
      kakyoin_text,
      solid_draw_meshes: Vec::new(),
      semi_transparent_draw_meshes: Vec::new(),
      drawing_bbs: false,
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
    self.crosshair[0]
      .0
      .set_position(crosshair_position + Vector3::new(0.5, 0.0, 0.0));
    self.crosshair[1]
      .0
      .set_position(crosshair_position + Vector3::new(0.0, 0.5, 0.0));
    self.crosshair[2]
      .0
      .set_position(crosshair_position + Vector3::new(0.0, 0.0, 0.5));

    let angle = 0.04 * time_since_last_update.as_secs_f32();
    // rotate around y axis
    let rotation = quaternion_from_angle(angle, Vector3::new(0.0, 1.0, 0.0));

    // todo: implement more efficient aabb recalculation for axis rotations
    self.kakyoin.obj_mut().unwrap_obj_mut().rotate(rotation);
    self
      .kakyoin
      .recalculate_bounding_boxes(&mut self.child_nodes);
    self.niko.node.obj_mut().unwrap_obj_mut().rotate(rotation);
    self
      .niko
      .node
      .recalculate_bounding_boxes(&mut self.child_nodes);

    // make text gradually rotate towards the camera
    let rotate_amount = 2.0 * time_since_last_update.as_secs_f32();
    self
      .niko_text
      .linearly_vertically_rotate_to_point(self.camera.position(), rotate_amount);
    self
      .kakyoin_text
      .linearly_vertically_rotate_to_point(self.camera.position(), rotate_amount);

    let camera_frustum = self.camera.create_frustum();

    self.solid_draw_meshes.clear();
    self.semi_transparent_draw_meshes.clear();
    self.niko.add_draw_meshes(
      &self.child_nodes,
      &self.last_update_projection_view,
      &camera_frustum,
      &mut self.solid_draw_meshes,
      &mut self.semi_transparent_draw_meshes,
    );
    self.kakyoin.add_draw_meshes(
      &self.child_nodes,
      self.last_update_projection_view,
      &camera_frustum,
      &mut self.solid_draw_meshes,
      &mut self.semi_transparent_draw_meshes,
    );
    self.test_cube.add_draw_meshes(
      &self.child_nodes,
      self.last_update_projection_view,
      &camera_frustum,
      &mut self.solid_draw_meshes,
      &mut self.semi_transparent_draw_meshes,
    );
    self.floor.add_draw_meshes(
      &self.child_nodes,
      self.last_update_projection_view,
      &camera_frustum,
      &mut self.solid_draw_meshes,
      &mut self.semi_transparent_draw_meshes,
    );
  }

  pub fn key_b_event(&mut self, pressed: bool) {
    if pressed {
      self.drawing_bbs = !self.drawing_bbs;
    }
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
