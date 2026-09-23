use std::ops::BitXor;

use winit::{event::ElementState, keyboard::KeyCode};

/// State of each key
#[derive(PartialEq, Clone, Copy)]
pub enum KeyState {
  Pressed,
  Released,
}

impl Default for KeyState {
  fn default() -> Self {
    KeyState::Released
  }
}

impl BitXor for KeyState {
  type Output = bool;

  fn bitxor(self, rhs: Self) -> Self::Output {
    Into::<bool>::into(self) ^ Into::<bool>::into(rhs)
  }
}

impl From<ElementState> for KeyState {
  fn from(state: ElementState) -> Self {
    match state {
      ElementState::Pressed => Self::Pressed,
      ElementState::Released => Self::Released,
    }
  }
}

impl Into<bool> for KeyState {
  fn into(self) -> bool {
    match self {
      Self::Pressed => true,
      Self::Released => false,
    }
  }
}

/// struct that contains information about each pressed / not pressed key
#[derive(Default)]
pub struct Keys {
  pub a: KeyState,
  pub w: KeyState,
  pub s: KeyState,
  pub d: KeyState,
  pub q: KeyState,
  pub e: KeyState,
  pub space: KeyState,
  pub l_shift: KeyState,
  pub l_ctrl: KeyState,
  pub up_key: KeyState,
  pub down_key: KeyState,
  pub left_key: KeyState,
  pub right_key: KeyState,
}

impl Keys {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn update_from_event(&mut self, code: KeyCode, state: ElementState) {
    let s = KeyState::from(state);
    match code {
      KeyCode::KeyA => self.a = s,
      KeyCode::KeyW => self.w = s,
      KeyCode::KeyS => self.s = s,
      KeyCode::KeyD => self.d = s,
      KeyCode::KeyQ => self.q = s,
      KeyCode::KeyE => self.e = s,
      KeyCode::Space => self.space = s,
      KeyCode::ShiftLeft => self.l_shift = s,
      KeyCode::ControlLeft => self.l_ctrl = s,
      KeyCode::ArrowUp => self.up_key = s,
      KeyCode::ArrowDown => self.down_key = s,
      KeyCode::ArrowLeft => self.left_key = s,
      KeyCode::ArrowRight => self.right_key = s,
      _ => {}
    }
  }
}
