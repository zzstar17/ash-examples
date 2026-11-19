use cgmath::{Matrix4, Point3};
use itertools::Itertools;

use crate::{
  asset_loader::{model_loader::AssetLocationData, Mesh},
  render::GraphicsPushConstants,
  scene::{frustum::Frustum, Render3dObj, SceneNode},
};

#[derive(Debug)]
pub struct Niko {
  pub node: usize,
  pub position: Point3<f32>,
}

impl Niko {
  pub fn new(
    asset_locations: &AssetLocationData,
    scene_collection: &gltf::Node,
    transform: &Matrix4<f32>,
    nodes: &mut Vec<SceneNode>,
  ) -> Self {
    let niko_collection_node = scene_collection
      .children()
      .find(|node| {
        if let Some(name) = node.name() {
          return name == "Niko";
        }
        false
      })
      .expect("Scene does not contain Niko");
    let (_niko_armature_i, _) = niko_collection_node
      .children()
      .find_position(|node| node.name().unwrap() == "Niko_Armature")
      .unwrap();
    let (_niko_lightbulb_i, _) = niko_collection_node
      .children()
      .find_position(|node| node.name().unwrap() == "Niko_Lightbulb")
      .unwrap();

    let (niko, niko_m) = SceneNode::from_gltf(
      asset_locations,
      niko_collection_node,
      None,
      Some(transform),
      None,
      nodes,
    );

    Self {
      node: niko,
      position: Render3dObj::from_model_matrix(niko_m).position(),
    }
  }

  pub fn add_draw_meshes(
    &self,
    child_nodes: &[SceneNode],
    projection_view: &Matrix4<f32>,
    camera_frustum: &Frustum,
    solid: &mut Vec<(GraphicsPushConstants, Mesh)>,
    semi_transparent: &mut Vec<(GraphicsPushConstants, Mesh)>,
  ) {
    SceneNode::add_draw_meshes(
      self.node,
      child_nodes,
      *projection_view,
      camera_frustum,
      solid,
      semi_transparent,
    );
  }
}
