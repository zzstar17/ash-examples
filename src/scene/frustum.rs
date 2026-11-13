use cgmath::{EuclideanSpace, InnerSpace, Point3, Vector3};

#[derive(Debug, Clone, Copy)]
pub struct Plane {
  pub normal: Vector3<f32>,
  // distance to origin
  pub distance: f32,
}

impl Plane {
  pub fn new(point: Point3<f32>, normal: Vector3<f32>) -> Self {
    let normal = normal.normalize();
    Self {
      normal,
      distance: normal.dot(point.to_vec()),
    }
  }

  pub fn get_signed_distance_to_plane(&self, point: Point3<f32>) -> f32 {
    self.normal.dot(point.to_vec()) - self.distance
  }
}

pub struct Frustum {
  pub top_face: Plane,
  pub bottom_face: Plane,
  pub right_face: Plane,
  pub left_face: Plane,
  pub far_face: Plane,
  pub near_face: Plane,
}
