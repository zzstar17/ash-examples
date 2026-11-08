use cgmath::Matrix4;
use itertools::Itertools;

use crate::{
  asset_loader::{model_loader::AssetLocationData, Mesh},
  render::GraphicsPushConstants,
  scene::{scene_node::ObjEnum, SceneNode},
};

#[derive(Debug)]
pub struct Niko {
  _body: usize,
  _lightbulb: usize,
  pub node: SceneNode,
}

impl Niko {
  pub fn new(
    asset_locations: &AssetLocationData,
    scene_collection: &gltf::Node,
    transform: &Matrix4<f32>,
    child_nodes: &mut Vec<SceneNode>,
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
    let (niko_armature_i, _) = niko_collection_node
      .children()
      .find_position(|node| node.name().unwrap() == "Niko_Armature")
      .unwrap();
    let (niko_lightbulb_i, _) = niko_collection_node
      .children()
      .find_position(|node| node.name().unwrap() == "Niko_Lightbulb")
      .unwrap();

    let niko = SceneNode::from_gltf(
      asset_locations,
      niko_collection_node,
      None,
      Some(ObjEnum::Matrix(*transform)),
      true,
      child_nodes,
    );
    let niko_body = niko.unwrap_parent().children[niko_armature_i];
    let niko_lightbulb = niko.unwrap_parent().children[niko_lightbulb_i];
    Self {
      _body: niko_body,
      _lightbulb: niko_lightbulb,
      node: niko,
    }
  }

  pub fn add_draw_meshes(
    &self,
    child_nodes: &[SceneNode],
    projection_view: &Matrix4<f32>,
    solid: &mut Vec<(GraphicsPushConstants, Mesh)>,
    semi_transparent: &mut Vec<(GraphicsPushConstants, Mesh)>,
  ) {
    self
      .node
      .add_draw_meshes(child_nodes, *projection_view, solid, semi_transparent);
  }
}
