use cgmath::Matrix4;

use crate::{
  asset_loader::{model_loader::AssetLocationData, Mesh},
  render::GraphicsPushConstants,
  scene::obj_3d::Render3dObj,
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

impl SceneNode {
  pub fn from_obj(obj: Render3dObj, mesh: Mesh) -> Self {
    SceneNode {
      obj: ObjEnum::Obj(obj),
      children: Vec::new(),
      mesh: Some(mesh),
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
