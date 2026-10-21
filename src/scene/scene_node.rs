use cgmath::{Point3, Quaternion, Vector3};
use gltf::Gltf;

use crate::{
  asset_loader::{LoadedModels, Mesh},
  scene::{ferris::Ferris, obj_3d::Render3dObj},
};

#[derive(Debug, Clone)]
pub struct SceneNode {
  // either has own render position, or inherits it
  obj: Render3dObj,
  children: Vec<usize>,
  mesh: Mesh,
}

pub struct LocalNodes {
  pub ferris: SceneNode,
  pub ferris_borders: [SceneNode; 4],
  pub crosshair: [SceneNode; 3],
}

impl SceneNode {
  fn from_obj(obj: Render3dObj, mesh: Mesh) -> Self {
    SceneNode {
      obj,
      children: Vec::new(),
      mesh,
    }
  }

  // not loaded from files
  fn local_nodes(loaded_models: &LoadedModels, ferris: &Ferris) -> LocalNodes {
    let quad_mesh = loaded_models.meshes[loaded_models.quad_mesh_index];
    let cube_mesh = loaded_models.meshes[loaded_models.cube_mesh_index];

    let ferris = Self::from_obj(
      Render3dObj::from_full(
        Point3::new(ferris.pos[0], ferris.pos[1], -3.0),
        Quaternion::new(1.0, 0.0, 0.0, 0.0),
        Vector3::new(Ferris::WIDTH, Ferris::HEIGHT, 1.0),
      ),
      quad_mesh,
    );

    let ferris_borders = [
      Self::from_obj(
        Render3dObj::from_full(
          Point3::new(0.5, 1.0 + 0.066, -2.95),
          Quaternion::new(1.0, 0.0, 0.0, 0.0),
          Vector3::new(0.59, 0.03, 1.0),
        ),
        quad_mesh,
      ),
      Self::from_obj(
        Render3dObj::from_full(
          Point3::new(0.5, 0.0 - 0.066, -2.95),
          Quaternion::new(1.0, 0.0, 0.0, 0.0),
          Vector3::new(0.59, 0.03, 1.0),
        ),
        quad_mesh,
      ),
      Self::from_obj(
        Render3dObj::from_full(
          Point3::new(0.0 - 0.066, 0.5, -2.95),
          Quaternion::new(1.0, 0.0, 0.0, 0.0),
          Vector3::new(0.03, 0.59, 1.0),
        ),
        quad_mesh,
      ),
      Self::from_obj(
        Render3dObj::from_full(
          Point3::new(1.0 + 0.066, 0.5, -2.95),
          Quaternion::new(1.0, 0.0, 0.0, 0.0),
          Vector3::new(0.03, 0.59, 1.0),
        ),
        quad_mesh,
      ),
    ];
    let crosshair = [
      Self::from_obj(
        Render3dObj::from_full(
          Point3::new(0.5, 0.0, 0.0),
          Quaternion::new(1.0, 0.0, 0.0, 0.0),
          Vector3::new(0.5, 0.05, 0.05),
        ),
        cube_mesh,
      ),
      Self::from_obj(
        Render3dObj::from_full(
          Point3::new(0.0, 0.5, 0.0),
          Quaternion::new(1.0, 0.0, 0.0, 0.0),
          Vector3::new(0.05, 0.5, 0.05),
        ),
        cube_mesh,
      ),
      Self::from_obj(
        Render3dObj::from_full(
          Point3::new(0.0, 0.0, 0.5),
          Quaternion::new(1.0, 0.0, 0.0, 0.0),
          Vector3::new(0.05, 0.05, 0.5),
        ),
        cube_mesh,
      ),
    ];

    LocalNodes {
      ferris,
      ferris_borders,
      crosshair,
    }
  }

  pub fn from_gltf_skin(loaded_models: &LoadedModels, skin: gltf::Skin, join: &mut Vec<Self>) {
    // todo
  }
}
