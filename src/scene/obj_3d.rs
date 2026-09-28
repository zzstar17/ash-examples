use cgmath::{InnerSpace, Matrix4, Point3, Quaternion, Vector3, Vector4};

/// Object information suitable for rendering in 3D. Caches certain matrices
/// in order to perform less calculations while rendering.
#[derive(Debug, Clone, Copy)]
pub struct Render3dObj {
  pub rotation: Quaternion<f32>,
  pub scale: Vector3<f32>,
  pub model_matrix: Matrix4<f32>,
}

fn compose_model_matrix(
  position: Point3<f32>,
  rotation: Quaternion<f32>,
  scale: Vector3<f32>,
) -> Matrix4<f32> {
  let x2 = rotation.v.x + rotation.v.x;
  let y2 = rotation.v.y + rotation.v.y;
  let z2 = rotation.v.z + rotation.v.z;

  let xx2 = x2 * rotation.v.x;
  let xy2 = x2 * rotation.v.y;
  let xz2 = x2 * rotation.v.z;

  let yy2 = y2 * rotation.v.y;
  let yz2 = y2 * rotation.v.z;
  let zz2 = z2 * rotation.v.z;

  let sy2 = y2 * rotation.s;
  let sz2 = z2 * rotation.s;
  let sx2 = x2 * rotation.s;

  Matrix4::from_cols(
    Vector4::new(
      (1.0 - yy2 - zz2) * scale.x,
      (xy2 + sz2) * scale.x,
      (xz2 - sy2) * scale.x,
      0.0,
    ),
    Vector4::new(
      (xy2 - sz2) * scale.y,
      (1.0 - xx2 - zz2) * scale.y,
      (yz2 + sx2) * scale.y,
      0.0,
    ),
    Vector4::new(
      (xz2 + sy2) * scale.z,
      (yz2 - sx2) * scale.z,
      (1.0 - xx2 - yy2) * scale.z,
      0.0,
    ),
    Vector4::new(position.x, position.y, position.z, 1.0),
  )
}

#[allow(dead_code)]
impl Render3dObj {
  pub fn new(position: Point3<f32>) -> Self {
    let rotation = Quaternion {
      v: Vector3::new(0.0, 0.0, 0.0),
      s: 1.0,
    };

    let scale = 1.0;
    let scale = Vector3::new(scale, scale, scale);

    Self {
      rotation,
      scale: scale,
      model_matrix: compose_model_matrix(position, rotation, scale),
    }
  }

  pub fn from_full(position: Point3<f32>, rotation: Quaternion<f32>, scale: Vector3<f32>) -> Self {
    debug_assert!((rotation.magnitude2() - 1.0).abs() < 0.001);

    Self {
      rotation,
      scale,
      model_matrix: compose_model_matrix(position, rotation, scale),
    }
  }

  pub fn model(&self) -> Matrix4<f32> {
    self.model_matrix
  }

  pub fn position(&self) -> Point3<f32> {
    Point3 {
      x: self.model_matrix.w.x,
      y: self.model_matrix.w.y,
      z: self.model_matrix.w.z,
    }
  }

  pub fn move_y(&mut self, relative: f32) {
    self.model_matrix.w.y += relative;
  }

  pub fn set_position(&mut self, new_position: Point3<f32>) {
    self.model_matrix.w.x = new_position.x;
    self.model_matrix.w.y = new_position.y;
    self.model_matrix.w.z = new_position.z;
  }

  pub fn set_rotation(&mut self, new_rotation: Quaternion<f32>) {
    self.rotation = new_rotation;
    debug_assert!((self.rotation.magnitude2() - 1.0).abs() < 0.001);

    self.update_model_matrix_full();
  }

  pub fn rotate(&mut self, rotation: Quaternion<f32>) {
    debug_assert!((rotation.magnitude2() - 1.0).abs() < 0.001);
    self.rotation = rotation * self.rotation;
    debug_assert!((self.rotation.magnitude2() - 1.0).abs() < 0.001);

    self.update_model_matrix_full();
  }

  fn update_model_matrix_full(&mut self) {
    self.model_matrix = compose_model_matrix(self.position(), self.rotation, self.scale);
  }
}
