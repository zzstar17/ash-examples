use std::mem::size_of;

use super::vertices::Vertex;

pub static VERTICES: [Vertex; 4] = [
  // top left
  Vertex {
    pos: [0.0, 0.0],
    tex_coords: [0.0, 0.0],
  },
  // bottom left
  Vertex {
    pos: [2.0, 0.0],
    tex_coords: [1.0, 0.0],
  },
  // top right
  Vertex {
    pos: [0.0, 2.0],
    tex_coords: [0.0, 1.0],
  },
  // bottom right
  Vertex {
    pos: [2.0, 2.0],
    tex_coords: [1.0, 1.0],
  },
];
pub static VERTICES_SIZE: u64 = (size_of::<Vertex>() * VERTICES.len()) as u64;

pub static QUAD_INDICES: [u16; 6] = [0, 1, 2, 3, 2, 1];
pub static QUAD_INDICES_SIZE: u64 = (size_of::<u16>() * QUAD_INDICES.len()) as u64;

// represents a position of the object that will be rendered
#[repr(C)]
pub struct RenderPosition {
  position: [f32; 2],
  ratio: [f32; 2], // width and height in relation to the surface
}

impl RenderPosition {
  pub fn new(position: [f32; 2], ratio: [f32; 2]) -> Self {
    Self { position, ratio }
  }
}
