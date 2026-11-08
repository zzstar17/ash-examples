use cgmath::{Quaternion, Vector3};

mod aabb;
mod camera;
mod niko;
mod obj_3d;
mod scene;
mod scene_node;

pub use aabb::BoundingBox;
pub use obj_3d::Render3dObj;
pub use scene::Scene;
pub use scene_node::SceneNode;

const UP: Vector3<f32> = Vector3::new(0.0, 1.0, 0.0);

const CAMERA_MOUSE_SENSITIVITY: f64 = 0.006;
const CAMERA_KEYBOARD_SENSITIVITY: f32 = 2.0;

// axis should be normalized
fn quaternion_from_angle(angle: f32, axis_of_rotation: Vector3<f32>) -> Quaternion<f32> {
  Quaternion {
    s: (angle / 2.0).cos(),
    v: (angle / 2.0).sin() * axis_of_rotation,
  }
}
