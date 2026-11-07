use cgmath::Matrix4;

use crate::{
  asset_loader::{model_loader::AssetLocationData, Mesh},
  render::GraphicsPushConstants,
  scene::{obj_3d::Render3dObj, BoundingBox},
};

#[derive(Debug, Clone, Copy)]
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

  pub fn matrix(&self) -> &Matrix4<f32> {
    match self {
      ObjEnum::Obj(obj) => obj.model(),
      ObjEnum::Matrix(m) => m,
    }
  }
}

#[derive(Debug, Clone)]
pub enum SceneNode {
  Parent(SceneNodeParent),
  Leaf(SceneNodeLeaf),
}

#[derive(Debug, Clone)]
pub struct SceneNodeParent {
  pub obj: ObjEnum,

  pub world_bb: BoundingBox,
  pub children: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct SceneNodeLeaf {
  pub obj: Option<ObjEnum>,

  pub world_bb: BoundingBox,
  pub mesh: Mesh,
}

impl SceneNode {
  pub fn unwrap_leaf(&self) -> &SceneNodeLeaf {
    match self {
      Self::Leaf(l) => l,
      Self::Parent(_p) => panic!(),
    }
  }

  pub fn unwrap_parent(&self) -> &SceneNodeParent {
    match self {
      Self::Leaf(_l) => panic!(),
      Self::Parent(p) => p,
    }
  }

  pub fn obj(&self) -> &ObjEnum {
    match self {
      Self::Parent(p) => &p.obj,
      Self::Leaf(l) => l.obj.as_ref().unwrap(),
    }
  }

  // pub fn obj_mut(&mut self) -> &mut ObjEnum {
  //   match self {
  //     Self::Parent(p) => &mut p.obj,
  //     Self::Leaf(l) => l.obj.as_mut().unwrap(),
  //   }
  // }

  pub fn world_bb(&self) -> BoundingBox {
    match self {
      Self::Parent(p) => p.world_bb,
      Self::Leaf(l) => l.world_bb,
    }
  }

  pub fn from_obj(obj: Render3dObj, mesh: Mesh) -> Self {
    let world_bb = mesh.local_bb.apply_matrix4(&obj.model_matrix);

    Self::Leaf(SceneNodeLeaf {
      obj: Some(ObjEnum::Obj(obj)),
      world_bb,
      mesh,
    })
  }

  pub fn from_gltf(
    assets: &AssetLocationData,
    node: gltf::Node,
    world_transform: Option<&Matrix4<f32>>,
    parent_transform: Option<ObjEnum>,
    modifiable: bool,
    join: &mut Vec<Self>,
  ) -> Self {
    let obj = if modifiable {
      if let Some(parent_t) = parent_transform {
        let m = Matrix4::from(node.transform().matrix());
        ObjEnum::Obj(Render3dObj::from_model_matrix(parent_t.matrix() * m))
      } else {
        ObjEnum::Obj(Render3dObj::from_gltf_transform(node.transform()))
      }
    } else {
      let m = Matrix4::from(node.transform().matrix());
      if let Some(parent_t) = parent_transform {
        ObjEnum::Matrix(parent_t.matrix() * m)
      } else {
        ObjEnum::Matrix(m)
      }
    };

    // todo: 2 comes from square and cube mesh
    let mesh_opt = node
      .mesh()
      .map(|gltf_mesh| assets.meshes[gltf_mesh.index() + 2]);

    let children_len = node.children().len();
    if children_len == 0 {
      let mesh = mesh_opt.expect("Scene contains leaf node without mesh");

      let new_world_transform = if let Some(transform) = world_transform {
        transform * obj.matrix()
      } else {
        *obj.matrix()
      };
      let world_bb = mesh.local_bb.apply_matrix4(&new_world_transform);

      Self::Leaf(SceneNodeLeaf {
        obj: Some(obj),
        world_bb,
        mesh,
      })
    } else if mesh_opt.is_none() && children_len == 1 {
      Self::from_gltf(
        assets,
        node.children().next().unwrap(),
        world_transform,
        Some(obj),
        false,
        join,
      )
    } else {
      let new_world_transform = if let Some(transform) = world_transform {
        transform * obj.matrix()
      } else {
        *obj.matrix()
      };

      let mut children;
      let mut children_iter = node.children();
      let mut world_bb = if let Some(mesh) = mesh_opt {
        children = Vec::with_capacity(children_len + 1);

        let world_bb = mesh.local_bb.apply_matrix4(&new_world_transform);
        let new = Self::Leaf(SceneNodeLeaf {
          obj: None,
          mesh,
          world_bb,
        });
        join.push(new);
        children.push(join.len() - 1);

        world_bb
      } else {
        children = Vec::with_capacity(children_len);

        // add first child
        let new = Self::from_gltf(
          assets,
          children_iter.next().unwrap(),
          Some(&new_world_transform),
          None,
          false,
          join,
        );
        let bb = new.world_bb();
        join.push(new);
        children.push(join.len() - 1);

        bb
      };

      for child in children_iter {
        let new = Self::from_gltf(assets, child, Some(&new_world_transform), None, false, join);
        let bb = new.world_bb();
        join.push(new);
        children.push(join.len() - 1);

        world_bb = world_bb.merge_with(&bb)
      }
      Self::Parent(SceneNodeParent {
        obj,
        children,
        world_bb,
      })
    }
  }

  // pub fn apply_transform_matrix(&mut self, m: &Matrix4<f32>) {
  //   let obj = match self {
  //     Self::Parent(p) => &mut p.obj,
  //     Self::Leaf(l) => l.obj.as_mut().unwrap(),
  //   };

  //   match obj {
  //     ObjEnum::Matrix(own) => *own = m * *own,
  //     ObjEnum::Obj(obj) => *obj = Render3dObj::from_model_matrix(m * obj.model()),
  //   }
  // }

  pub fn add_draw_meshes(
    &self,
    nodes: &[SceneNode],
    parent_transform: Matrix4<f32>,
    solid: &mut Vec<(GraphicsPushConstants, Mesh)>,
    semi_transparent: &mut Vec<(GraphicsPushConstants, Mesh)>,
  ) {
    match self {
      Self::Leaf(leaf) => {
        let transformed;
        if let Some(obj) = leaf.obj.as_ref() {
          let model = match obj {
            ObjEnum::Matrix(m) => *m,
            ObjEnum::Obj(obj) => *obj.model(),
          };
          transformed = parent_transform * model;
        } else {
          transformed = parent_transform;
        }

        let pc = GraphicsPushConstants {
          matrix: transformed,
        };
        if leaf.mesh.semi_transparent {
          semi_transparent.push((pc, leaf.mesh));
        } else {
          solid.push((pc, leaf.mesh));
        }
      }
      Self::Parent(par) => {
        let model = match par.obj {
          ObjEnum::Matrix(m) => m,
          ObjEnum::Obj(obj) => *obj.model(),
        };
        let transformed = parent_transform * model;

        for &child in par.children.iter() {
          nodes[child].add_draw_meshes(nodes, transformed, solid, semi_transparent);
        }
      }
    }
  }
}
