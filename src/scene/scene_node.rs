use cgmath::{Matrix4, Point3, Quaternion, Vector3};

use crate::{
  asset_loader::{model_loader::AssetLocationData, LoadedModels, Mesh},
  render::GraphicsPushConstants,
  scene::{ferris::Ferris, obj_3d::Render3dObj},
};

#[derive(Debug, Clone)]
pub enum ObjEnum {
  Obj(Render3dObj),
  Matrix(Matrix4<f32>),
}

impl ObjEnum {
  pub fn unwrap_obj(&self) -> &Render3dObj {
    match self {
      ObjEnum::Obj(obj) => obj,
      ObjEnum::Matrix(_) => panic!(),
    }
  }

  pub fn unwrap_obj_mut(&mut self) -> &mut Render3dObj {
    match self {
      ObjEnum::Obj(obj) => obj,
      ObjEnum::Matrix(_) => panic!(),
    }
  }
}

#[derive(Debug, Clone)]
pub struct SceneNode {
  pub obj: ObjEnum,
  pub children: Vec<usize>,
  pub mesh: Option<Mesh>,
}

#[derive(Debug, Clone)]
pub struct SceneNodeLeaf {
  pub obj: Render3dObj,
  pub children: Vec<usize>,
  pub mesh: Mesh,
}

pub struct LocalNodes {
  pub ferris: SceneNode,
  pub ferris_borders: [SceneNode; 4],
  pub crosshair: [SceneNode; 3],
}

impl SceneNode {
  fn from_obj(obj: Render3dObj, mesh: Mesh) -> Self {
    SceneNode {
      obj: ObjEnum::Obj(obj),
      children: Vec::new(),
      mesh: Some(mesh),
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

  pub fn from_gltf(
    assets: &AssetLocationData,
    node: gltf::Node,
    modifiable: bool,
    join: &mut Vec<Self>,
  ) -> Self {
    let obj = if modifiable {
      ObjEnum::Obj(Render3dObj::from_gltf_transform(node.transform()))
    } else {
      ObjEnum::Matrix(Matrix4::from(node.transform().matrix()))
    };

    let mut children = Vec::with_capacity(node.children().len());
    for child in node.children() {
      let new = Self::from_gltf(assets, child, false, join);
      join.push(new);
      children.push(join.len() - 1);
    }

    // todo: 2 comes from square and cube mesh
    let mesh = node
      .mesh()
      .map(|gltf_mesh| assets.meshes[gltf_mesh.index() + 2]);

    Self {
      obj,
      children,
      mesh,
    }
  }

  pub fn apply_transform_matrix(&mut self, m: &Matrix4<f32>) {
    match &mut self.obj {
      ObjEnum::Matrix(own) => *own = m * *own,
      ObjEnum::Obj(obj) => *obj = Render3dObj::from_model_matrix(m * obj.model()),
    }
  }

  pub fn add_draw_meshes(
    &self,
    nodes: &[SceneNode],
    parent_transform: Matrix4<f32>,
    solid: &mut Vec<(GraphicsPushConstants, Mesh)>,
    semi_transparent: &mut Vec<(GraphicsPushConstants, Mesh)>,
  ) {
    let model = match self.obj {
      ObjEnum::Matrix(m) => m,
      ObjEnum::Obj(obj) => obj.model(),
    };
    let transformed = parent_transform * model;
    if let Some(mesh) = self.mesh {
      let pc = GraphicsPushConstants {
        matrix: transformed,
        tex_offset: [0.0, 0.0],
        tex_size: [1.0, 1.0],
      };
      if mesh.semi_transparent {
        semi_transparent.push((pc, mesh));
      } else {
        solid.push((pc, mesh));
      }
    }

    for &child in self.children.iter() {
      nodes[child].add_draw_meshes(nodes, transformed, solid, semi_transparent);
    }
  }
}
