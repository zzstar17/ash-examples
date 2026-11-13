use cgmath::{EuclideanSpace, Matrix4, Point2, Point3, Vector2, Vector3};

use crate::scene::{
  frustum::{Frustum, Plane},
  Render3dObj,
};

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
  pub center: Point3<f32>,
  pub halfwidth: Vector3<f32>,
}

impl Default for BoundingBox {
  fn default() -> Self {
    Self {
      center: Point3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
      },
      halfwidth: Vector3 {
        x: 0.0,
        y: 0.0,
        z: 0.0,
      },
    }
  }
}

fn rotate_point_around_point(
  rotation_center: Point2<f32>,
  point_to_rotate: Point2<f32>,
  angle_cos: f32,
  angle_sin: f32,
) -> Point2<f32> {
  let shifted = point_to_rotate - rotation_center;
  let rotated = Vector2::new(
    shifted.x * angle_cos - shifted.y * angle_sin,
    shifted.y * angle_cos - shifted.x * angle_sin,
  );
  rotation_center + rotated
}

impl BoundingBox {
  pub fn from_gltf(bb: gltf::mesh::BoundingBox) -> Self {
    let min = Point3::from(bb.min);
    let max = Point3::from(bb.max);
    let halfwidth = (max - min) * 0.5;
    let center = min + halfwidth;
    Self { center, halfwidth }
  }

  pub fn apply_matrix4(&self, matrix: &Matrix4<f32>) -> Self {
    let mut new_center = Point3 {
      x: matrix.w.x,
      y: matrix.w.y,
      z: matrix.w.z,
    };
    let mut new_halfwidth = Vector3::new(0.0, 0.0, 0.0);
    for i in 0..3 {
      for j in 0..3 {
        new_center[i] += matrix[i][j] * self.center[j];
        new_halfwidth[i] += (matrix[i][j]).abs() * self.halfwidth[j];
      }
    }
    Self {
      center: new_center,
      halfwidth: new_halfwidth,
    }
  }

  pub fn merge_with(&self, other: &Self) -> Self {
    let min = Vector3::new(
      (self.center.x - self.halfwidth.x).min(other.center.x - other.halfwidth.x),
      (self.center.y - self.halfwidth.y).min(other.center.y - other.halfwidth.y),
      (self.center.z - self.halfwidth.z).min(other.center.z - other.halfwidth.z),
    );
    let max = Vector3::new(
      (self.center.x + self.halfwidth.x).max(other.center.x + other.halfwidth.x),
      (self.center.y + self.halfwidth.y).max(other.center.y + other.halfwidth.y),
      (self.center.z + self.halfwidth.z).max(other.center.z + other.halfwidth.z),
    );

    let halfwidth = (max - min) / 2.0;
    let center = Point3::from_vec(min + halfwidth);
    Self { center, halfwidth }
  }

  pub fn is_on_or_forward_plane(&self, plane: Plane) -> bool {
    let r = self.halfwidth.x * plane.normal.x.abs()
      + self.halfwidth.y * plane.normal.y.abs()
      + self.halfwidth.z * plane.normal.z.abs();
    -r <= plane.get_signed_distance_to_plane(self.center)
  }

  pub fn is_on_frustum(&self, frustum: &Frustum) -> bool {
    self.is_on_or_forward_plane(frustum.left_face)
      && self.is_on_or_forward_plane(frustum.right_face)
      && self.is_on_or_forward_plane(frustum.top_face)
      && self.is_on_or_forward_plane(frustum.bottom_face)
      && self.is_on_or_forward_plane(frustum.near_face)
      && self.is_on_or_forward_plane(frustum.far_face)
  }

  pub fn generate_box_visualization(&self, line_width: f32, buffer: &mut Vec<Matrix4<f32>>) {
    {
      let x_axis = Render3dObj::from_position_scale(
        self.center,
        Vector3 {
          x: self.halfwidth.x,
          y: line_width,
          z: line_width,
        },
      );

      let mut x = [x_axis, x_axis, x_axis, x_axis];
      x[0].move_(Vector3::new(0.0, self.halfwidth.y, self.halfwidth.z));
      x[1].move_(Vector3::new(0.0, -self.halfwidth.y, self.halfwidth.z));
      x[2].move_(Vector3::new(0.0, self.halfwidth.y, -self.halfwidth.z));
      x[3].move_(Vector3::new(0.0, -self.halfwidth.y, -self.halfwidth.z));
      for obj in x {
        let m = obj.model_matrix;
        buffer.push(m);
      }
    }

    {
      let y_axis = Render3dObj::from_position_scale(
        self.center,
        Vector3 {
          x: line_width,
          y: self.halfwidth.y,
          z: line_width,
        },
      );

      let mut y = [y_axis, y_axis, y_axis, y_axis];
      y[0].move_(Vector3::new(self.halfwidth.x, 0.0, self.halfwidth.z));
      y[1].move_(Vector3::new(-self.halfwidth.x, 0.0, self.halfwidth.z));
      y[2].move_(Vector3::new(self.halfwidth.x, 0.0, -self.halfwidth.z));
      y[3].move_(Vector3::new(-self.halfwidth.x, 0.0, -self.halfwidth.z));
      for obj in y {
        let m = obj.model_matrix;
        buffer.push(m);
      }
    }

    {
      let z_axis = Render3dObj::from_position_scale(
        self.center,
        Vector3 {
          x: line_width,
          y: line_width,
          z: self.halfwidth.z,
        },
      );

      let mut z = [z_axis, z_axis, z_axis, z_axis];
      z[0].move_(Vector3::new(self.halfwidth.x, self.halfwidth.y, 0.0));
      z[1].move_(Vector3::new(-self.halfwidth.x, self.halfwidth.y, 0.0));
      z[2].move_(Vector3::new(self.halfwidth.x, -self.halfwidth.y, 0.0));
      z[3].move_(Vector3::new(-self.halfwidth.x, -self.halfwidth.y, 0.0));
      for obj in z {
        let m = obj.model_matrix;
        buffer.push(m);
      }
    }
  }
}
