use cgmath::{EuclideanSpace, Matrix4, Point3, Quaternion, Rotation};

use crate::{
  asset_loader::{model_loader::AssetLocationData, Mesh},
  render::GraphicsPushConstants,
  scene::{frustum::Frustum, BoundingBox, Render3dObj},
};

#[derive(Debug, Clone, Copy)]
pub struct SceneNode {
  pub world_bb: BoundingBox,

  // Always None on parent and Some on leaf
  pub leaf_data: Option<LeafData>,

  // next brother index in node list
  pub next_brother: Option<usize>,
  pub parent: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct LeafData {
  pub mesh: Mesh,
  pub obj: Render3dObj,
}

impl Default for SceneNode {
  fn default() -> Self {
    Self {
      world_bb: BoundingBox::default(),
      leaf_data: None,
      next_brother: None,
      parent: None,
    }
  }
}

impl SceneNode {
  pub fn from_obj(obj: Render3dObj, mesh: Mesh) -> Self {
    let world_bb = mesh.local_bb.apply_matrix4(obj.model());

    Self {
      world_bb,
      leaf_data: Some(LeafData { mesh, obj }),
      next_brother: None,
      parent: None,
    }
  }

  pub fn from_gltf(
    assets: &AssetLocationData,
    node: gltf::Node,
    world_transform: Option<&Matrix4<f32>>,
    parent_transform: Option<&Matrix4<f32>>,
    parent: Option<usize>,
    node_stack: &mut Vec<Self>,
  ) -> (usize, Matrix4<f32>) {
    // local transform
    let m = Matrix4::from(node.transform().matrix());
    let local_transform = if let Some(parent_t) = parent_transform {
      parent_t * m
    } else {
      m
    };

    // todo: 2 comes from square and cube mesh
    let mesh_opt = node
      .mesh()
      .map(|gltf_mesh| assets.meshes[gltf_mesh.index() + 2]);

    let children_len = node.children().len();
    if children_len == 0 {
      let mesh = mesh_opt.expect("Scene contains leaf node without mesh");

      let new_world_transform = if let Some(transform) = world_transform {
        transform * local_transform
      } else {
        local_transform
      };
      let world_bb = mesh.local_bb.apply_matrix4(&new_world_transform);

      let leaf = Self {
        world_bb,
        leaf_data: Some(LeafData {
          mesh,
          obj: Render3dObj::from_model_matrix(new_world_transform),
        }),
        next_brother: None,
        parent,
      };
      let i = node_stack.len();
      node_stack.push(leaf);
      (i, new_world_transform)
    } else if mesh_opt.is_none() && children_len == 1 {
      // pass through
      Self::from_gltf(
        assets,
        node.children().next().unwrap(),
        world_transform,
        Some(&local_transform),
        parent,
        node_stack,
      )
    } else {
      let new_world_transform = if let Some(transform) = world_transform {
        transform * local_transform
      } else {
        local_transform
      };

      let parent_node = Self {
        world_bb: BoundingBox::default(), // overwritten later
        next_brother: None,
        leaf_data: None,
        parent,
      };
      let parent_i = node_stack.len();
      node_stack.push(parent_node);

      let mut last_child_i;
      let mut children_iter = node.children();
      let mut world_bb = if let Some(mesh) = mesh_opt {
        let world_bb = mesh.local_bb.apply_matrix4(&new_world_transform);
        let new = Self {
          world_bb,
          leaf_data: Some(LeafData {
            mesh,
            obj: Render3dObj::from_model_matrix(new_world_transform),
          }),
          next_brother: None,
          parent: Some(parent_i),
        };
        last_child_i = node_stack.len();
        node_stack.push(new);

        world_bb
      } else {
        // add first child
        let (new, _child_transform) = Self::from_gltf(
          assets,
          children_iter.next().unwrap(),
          Some(&new_world_transform),
          None,
          Some(parent_i),
          node_stack,
        );
        last_child_i = new;

        node_stack[new].world_bb
      };
      debug_assert_eq!(parent_i + 1, last_child_i);

      for child in children_iter {
        let (new, _child_transform) = Self::from_gltf(
          assets,
          child,
          Some(&new_world_transform),
          None,
          Some(parent_i),
          node_stack,
        );
        node_stack[last_child_i].next_brother = Some(new);
        last_child_i = new;

        world_bb = world_bb.merge_with(&node_stack[new].world_bb)
      }

      node_stack[parent_i].world_bb = world_bb;
      (parent_i, new_world_transform)
    }
  }

  // general but expensive
  /// Rotate around point and recalculate bounding boxes
  pub fn rotate_and_recalculate(
    node_i: usize,
    rotation: Quaternion<f32>,
    rotation_point: Point3<f32>,
    nodes: &mut [SceneNode],
  ) {
    let initial_node = node_i;
    let mut cur_i = node_i;
    let mut cur_node = &mut nodes[cur_i];

    loop {
      match cur_node.leaf_data.as_mut() {
        // leaf
        Some(leaf) => {
          let rotation_vec = rotation_point.to_vec();
          let translated_point =
            rotation.rotate_point(leaf.obj.position() - rotation_vec) + rotation_vec;
          leaf.obj.set_position(translated_point);
          leaf.obj.rotate(rotation);

          cur_node.world_bb = leaf.mesh.local_bb.apply_matrix4(leaf.obj.model());
        }
        // parent
        None => {
          cur_i += 1;
          cur_node = &mut nodes[cur_i];
          continue;
        }
      }

      loop {
        if let Some(brother) = cur_node.next_brother {
          cur_node = &mut nodes[brother];
          cur_i = brother;
          break;
        } else {
          if cur_i == initial_node {
            return;
          }
          let parent = cur_node.parent.unwrap();

          // recalculate parent bounding box
          let first_child = nodes[parent + 1];
          let mut parent_bb = first_child.world_bb;
          let mut cur_brother = first_child.next_brother;
          while let Some(brother) = cur_brother {
            parent_bb = parent_bb.merge_with(&nodes[brother].world_bb);
            cur_brother = nodes[brother].next_brother;
          }
          nodes[parent].world_bb = parent_bb;

          if parent == initial_node {
            return;
          } else {
            cur_node = &mut nodes[parent];
          }
        }
      }
    }
  }

  pub fn add_draw_meshes(
    node_i: usize,
    nodes: &[SceneNode],
    projection_view: Matrix4<f32>,
    camera_frustum: &Frustum,
    solid: &mut Vec<(GraphicsPushConstants, Mesh)>,
    semi_transparent: &mut Vec<(GraphicsPushConstants, Mesh)>,
  ) {
    let initial_node = node_i;
    let mut cur_i = node_i;
    let mut cur_node = &nodes[cur_i];
    loop {
      if cur_node.world_bb.is_on_frustum(camera_frustum) {
        match cur_node.leaf_data.as_ref() {
          // leaf
          Some(leaf) => {
            let transformed = projection_view * leaf.obj.model();

            let pc = GraphicsPushConstants {
              matrix: transformed,
            };
            if leaf.mesh.semi_transparent {
              semi_transparent.push((pc, leaf.mesh));
            } else {
              solid.push((pc, leaf.mesh));
            }
          }
          // parent
          None => {
            cur_i += 1;
            cur_node = &nodes[cur_i];
            continue;
          }
        }
      }

      loop {
        if let Some(brother) = cur_node.next_brother {
          cur_i = brother;
          cur_node = &nodes[brother];
          break;
        } else {
          if cur_i == initial_node {
            return;
          }
          let parent = cur_node.parent.unwrap();
          if parent == initial_node {
            return;
          } else {
            cur_node = &nodes[parent];
          }
        }
      }
    }
  }
}
