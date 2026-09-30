use std::f32::consts::PI;

use cgmath::{InnerSpace, Matrix4, Point3, Vector3, Vector4};

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

  pub fn new(speed: f32, yaw: f32, pitch: f32) -> Self {
    Self {
      position: Point3::new(0.0, 0.0, 0.0),
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
  /// normalized front vector
  front: Vector3<f32>,
  fov: f32,
  aspect_ratio: f32,
  /// cached because it's not frequently updated
  projection_matrix: Matrix4<f32>,
}

impl RenderCamera {
  const MAX_FOV: f32 = PI / 1.1;
  const MIN_FOV: f32 = PI / 15.0;
  const ZOOM_SENTIVITY: f32 = 0.07;

  pub fn new(camera: Camera, fov: f32, aspect_ratio: f32, sensitivity: f32) -> Self {
    let front = camera.get_front();

    Self {
      camera,
      front,
      fov,
      aspect_ratio,
      projection_matrix: calc_projection_matrix(fov, aspect_ratio),
    }
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
    Matrix4::look_at_rh(self.camera.position, self.camera.position + self.front, UP)
  }

  fn update_projection_matrix(&mut self) {
    self.projection_matrix = calc_projection_matrix(self.fov, self.aspect_ratio);
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
    self.fov = (self.fov + (amount * Self::ZOOM_SENTIVITY)).clamp(Self::MIN_FOV, Self::MAX_FOV);
    self.update_projection_matrix();
  }

  pub fn rotate(&mut self, delta_x: f32, delta_y: f32) {
    self.camera.yaw_relative(delta_x);
    self.camera.pitch_relative(delta_y);
    self.front = self.camera.get_front();
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
    self.camera.position -= self.front.cross(UP).normalize() * distance;
  }

  pub fn move_right(&mut self, duration: &std::time::Duration) {
    let distance = self.camera.speed * duration.as_secs_f32();
    self.camera.position += self.front.cross(UP).normalize() * distance;
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

fn calc_projection_matrix(fov: f32, aspect_ratio: f32) -> Matrix4<f32> {
  let far = 1000.0;
  let near = 0.05;

  let y_scale = 1.0 / f32::tan(fov * 0.5);
  let x_scale = y_scale / aspect_ratio;

  // right handed projection perspective fov matrix that keeps the deep range in [0, 1]
  // yoinked from https://learn.microsoft.com/en-us/windows/win32/direct3d9/d3dxmatrixperspectivefovrh
  // see https://github.com/rustgd/cgmath/issues/449
  let m = Matrix4 {
    x: Vector4::new(x_scale, 0.0, 0.0, 0.0),
    y: Vector4::new(0.0, y_scale, 0.0, 0.0),
    z: Vector4::new(0.0, 0.0, far / (near - far), -1.0),
    w: Vector4::new(0.0, 0.0, near * far / (near - far), 0.0),
  };

  m
}
