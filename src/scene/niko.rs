use cgmath::Matrix4;

use crate::{
  asset_loader::{model_loader::AssetLocationData, Mesh},
  render::GraphicsPushConstants,
  scene::{obj_3d::Render3dObj, SceneNode},
};

pub struct Niko {
  body: SceneNode,
  lightbulb: SceneNode,
  pub obj: Render3dObj,
}

impl Niko {
  pub fn new(
    asset_locations: &AssetLocationData,
    scene_collection: &gltf::Node,
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
    let niko_armature = niko_collection_node
      .children()
      .find(|node| node.name().unwrap() == "Niko_Armature")
      .unwrap();
    let niko_lightbulb = niko_collection_node
      .children()
      .find(|node| node.name().unwrap() == "Niko_Lightbulb")
      .unwrap();
    let niko_body = SceneNode::from_gltf(&asset_locations, niko_armature, true, child_nodes);
    let niko_lightbulb = SceneNode::from_gltf(&asset_locations, niko_lightbulb, true, child_nodes);
    Self {
      body: niko_body,
      lightbulb: niko_lightbulb,
      obj: Render3dObj::from_gltf_transform(niko_collection_node.transform()),
    }
  }

  pub fn add_draw_meshes(
    &self,
    child_nodes: &[SceneNode],
    projection_view: &Matrix4<f32>,
    solid: &mut Vec<(GraphicsPushConstants, Mesh)>,
    semi_transparent: &mut Vec<(GraphicsPushConstants, Mesh)>,
  ) {
    let transform = projection_view * self.obj.model();
    self
      .body
      .add_draw_meshes(&child_nodes, transform, solid, semi_transparent);
    self
      .lightbulb
      .add_draw_meshes(&child_nodes, transform, solid, semi_transparent);
  }
}
