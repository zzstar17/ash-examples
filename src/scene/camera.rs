use std::f32::consts::PI;

use cgmath::{EuclideanSpace, InnerSpace, Matrix4, Point3, Vector3, Vector4};

use crate::scene::frustum::{Frustum, Plane};

use super::UP;

const HALF_PI: f32 = PI / 2.0;

pub struct Camera {
  pub position: Point3<f32>,
  pub yaw: f32,
  pub pitch: f32,
  pub speed: f32,
}

impl Camera {
  const MAX_PITCH: f32 = HALF_PI - 0.1;
  const MIN_PITCH: f32 = -HALF_PI + 0.1;

  pub fn new(position: Point3<f32>, speed: f32, yaw: f32, pitch: f32) -> Self {
    Self {
      position,
      yaw,
      pitch,
      speed,
    }
  }

  pub fn get_front(&self) -> Vector3<f32> {
    let pitch_cos = self.pitch.cos();
    let x = self.yaw.cos() * pitch_cos;
    let y = self.pitch.sin();
    let z = self.yaw.sin() * pitch_cos;
    Vector3::new(x, y, z)
  }

  pub fn yaw_relative(&mut self, amount: f32) {
    self.yaw += amount;
  }

  pub fn pitch_relative(&mut self, amount: f32) {
    self.pitch = (self.pitch + amount).clamp(Self::MIN_PITCH, Self::MAX_PITCH);
  }
}

/// Camera with dynamic up / down directions.
pub struct RenderCamera {
  camera: Camera,
  /// normalized front, right and up vectors (from camera view)
  front: Vector3<f32>,
  right: Vector3<f32>,
  up: Vector3<f32>,

  fov: f32,
  inv_fov_tan: f32,
  aspect_ratio: f32,
  /// cached because it's not frequently updated
  projection_matrix: Matrix4<f32>,
}

impl RenderCamera {
  const MAX_FOV: f32 = PI / 1.1;
  const MIN_FOV: f32 = PI / 15.0;
  const ZOOM_SENSITIVITY: f32 = 0.07;

  const NEAR: f32 = 0.05;
  const FAR: f32 = 5000.0;

  pub fn new(camera: Camera, fov: f32, aspect_ratio: f32) -> Self {
    let front = camera.get_front();
    let right = front.cross(UP).normalize();
    let up = right.cross(front);

    let inv_fov_tan = Self::get_inv_fov_tan(fov);

    Self {
      camera,
      front,
      right,
      up,
      fov,
      inv_fov_tan,
      aspect_ratio,
      projection_matrix: calc_projection_matrix(inv_fov_tan, aspect_ratio, Self::NEAR, Self::FAR),
    }
  }

  pub fn create_frustum(&self) -> Frustum {
    let near = Self::NEAR;
    let far = Self::FAR;

    //let half_ver_side = far * (1.0 / self.inv_fov_tan);
    let half_ver_side = far * (self.fov * 0.5).tan();
    let half_hor_side = half_ver_side * self.aspect_ratio;
    let front_mult_far = far * self.front();

    let near_face = Plane::new(self.camera.position + near * self.front, self.front);
    let far_face = Plane::new(self.camera.position + front_mult_far, -self.front);

    let normal_right = front_mult_far - self.right * half_hor_side;
    let normal_left = front_mult_far + self.right * half_hor_side;
    let right_face = Plane::new(self.camera.position, normal_right.cross(self.up));
    let left_face = Plane::new(self.camera.position, self.up.cross(normal_left));

    let normal_top = front_mult_far - self.up * half_ver_side;
    let normal_bottom = front_mult_far + self.up * half_ver_side;
    let top_face = Plane::new(self.camera.position, self.right.cross(normal_top));
    let bottom_face = Plane::new(self.camera.position, normal_bottom.cross(self.right));

    Frustum {
      near_face,
      far_face,
      right_face,
      left_face,
      top_face,
      bottom_face,
    }
  }

  fn get_inv_fov_tan(fov: f32) -> f32 {
    1.0 / f32::tan(fov * 0.5)
  }

  pub fn speed_mut(&mut self) -> &mut f32 {
    &mut self.camera.speed
  }

  pub fn front(&self) -> Vector3<f32> {
    self.front
  }

  pub fn position(&self) -> Point3<f32> {
    self.camera.position
  }

  pub fn view_matrix(&self) -> Matrix4<f32> {
    let position = self.camera.position;

    Matrix4 {
      x: Vector4::new(self.right.x, self.up.x, -self.front.x, 0.0),
      y: Vector4::new(self.right.y, self.up.y, -self.front.y, 0.0),
      z: Vector4::new(self.right.z, self.up.z, -self.front.z, 0.0),
      w: Vector4::new(
        -position.dot(self.right),
        -position.dot(self.up),
        position.dot(self.front),
        1.0,
      ),
    }
  }

  fn update_projection_matrix(&mut self) {
    self.projection_matrix =
      calc_projection_matrix(self.inv_fov_tan, self.aspect_ratio, Self::NEAR, Self::FAR);
  }

  pub fn projection_matrix(&self) -> &Matrix4<f32> {
    &self.projection_matrix
  }

  pub fn projection_view(&self) -> Matrix4<f32> {
    self.projection_matrix() * self.view_matrix()
  }

  pub fn set_aspect_ratio(&mut self, value: f32) {
    self.aspect_ratio = value;
    self.update_projection_matrix();
  }

  pub fn zoom_relative(&mut self, amount: f32) {
    self.fov = (self.fov + (amount * Self::ZOOM_SENSITIVITY)).clamp(Self::MIN_FOV, Self::MAX_FOV);
    self.inv_fov_tan = Self::get_inv_fov_tan(self.fov);
    self.update_projection_matrix();
  }

  pub fn rotate(&mut self, delta_x: f32, delta_y: f32) {
    self.camera.yaw_relative(delta_x);
    self.camera.pitch_relative(delta_y);

    self.front = self.camera.get_front();
    self.right = self.front.cross(UP).normalize();
    self.up = self.right.cross(self.front);
  }

  pub fn move_forward(&mut self, duration: &std::time::Duration) {
    let distance = self.camera.speed * duration.as_secs_f32();

    let front_small = Vector3::new(self.front.x, 0.0, self.front.z);
    if front_small.magnitude2() > 0.001 {
      self.camera.position += front_small.normalize() * distance;
    }
  }

  pub fn move_backwards(&mut self, duration: &std::time::Duration) {
    let distance = self.camera.speed * duration.as_secs_f32();

    let front_small = Vector3::new(self.front.x, 0.0, self.front.z);
    if front_small.magnitude2() > 0.001 {
      self.camera.position -= front_small.normalize() * distance;
    }
  }

  pub fn move_left(&mut self, duration: &std::time::Duration) {
    let distance = self.camera.speed * duration.as_secs_f32();
    self.camera.position -= self.right * distance;
  }

  pub fn move_right(&mut self, duration: &std::time::Duration) {
    let distance = self.camera.speed * duration.as_secs_f32();
    self.camera.position += self.right * distance;
  }

  pub fn move_up(&mut self, duration: &std::time::Duration) {
    let distance = self.camera.speed * duration.as_secs_f32();
    self.camera.position -= UP * distance;
  }

  pub fn move_down(&mut self, duration: &std::time::Duration) {
    let distance = self.camera.speed * duration.as_secs_f32();
    self.camera.position += UP * distance;
  }
}

fn calc_projection_matrix(
  inv_fov_tan: f32,
  aspect_ratio: f32,
  near: f32,
  far: f32,
) -> Matrix4<f32> {
  let y_scale = inv_fov_tan;
  let x_scale = y_scale / aspect_ratio;

  // right handed projection perspective fov matrix that keeps the deep range in [0, 1]
  // yoinked from https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dxmatrixperspectivefovrh
  // see https://github.com/rustgd/cgmath/issues/449
  Matrix4 {
    x: Vector4::new(x_scale, 0.0, 0.0, 0.0),
    y: Vector4::new(0.0, y_scale, 0.0, 0.0),
    z: Vector4::new(0.0, 0.0, far / (near - far), -1.0),
    w: Vector4::new(0.0, 0.0, near * far / (near - far), 0.0),
  }
}
