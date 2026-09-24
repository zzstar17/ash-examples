use std::{fs::File, io::BufReader};

use crate::{render::TexturedVertex, NIKO_MODEL_PATH};

pub const QUAD_VERTICES: [TexturedVertex; 4] = [
  // top left
  TexturedVertex {
    pos: [-1.0, -1.0, 0.0],
    normal: [0.0, 0.0, 1.0],
    tex_coords: [0.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom left
  TexturedVertex {
    pos: [1.0, -1.0, 0.0],
    normal: [0.0, 0.0, 1.0],
    tex_coords: [1.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // top right
  TexturedVertex {
    pos: [-1.0, 1.0, 0.0],
    normal: [0.0, 0.0, 1.0],
    tex_coords: [0.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom right
  TexturedVertex {
    pos: [1.0, 1.0, 0.0],
    normal: [0.0, 0.0, 1.0],
    tex_coords: [1.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
];
pub const QUAD_VERTICES_SIZE: u64 = (size_of::<TexturedVertex>() * QUAD_VERTICES.len()) as u64;

pub const QUAD_INDICES: [u32; 6] = [0, 1, 2, 3, 2, 1];
pub const QUAD_INDICES_SIZE: u64 = (size_of::<u16>() * QUAD_INDICES.len()) as u64;

pub struct LoadedModels {
  pub vertices: Vec<TexturedVertex>,
  pub indices: Vec<u32>,

  pub models: Models,
}

#[derive(Debug, Clone, Copy)]
pub struct Models {
  pub quad: ModelOffset,
  pub niko: ModelOffset,
  pub total_index_count: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelOffset {
  pub vertices_offset: usize,
  pub indices_offset: usize,
  pub vertices_len: usize,
  pub indices_len: usize,
}

impl LoadedModels {
  pub fn load() -> Result<Self, obj::ObjError> {
    let niko_f = File::open(NIKO_MODEL_PATH)?;
    let mut buff_reader = BufReader::new(niko_f);

    let obj: obj::Obj<obj::TexturedVertex, u32> = obj::load_obj(&mut buff_reader)?;

    let quad_model = ModelOffset {
      vertices_offset: 0,
      indices_offset: 0,
      vertices_len: QUAD_VERTICES.len(),
      indices_len: QUAD_INDICES.len(),
    };

    let niko_model = ModelOffset {
      vertices_offset: quad_model.vertices_len,
      indices_offset: quad_model.indices_len,
      vertices_len: obj.vertices.len(),
      indices_len: obj.indices.len(),
    };

    let mut vertices = Vec::with_capacity(quad_model.vertices_len + niko_model.indices_len);
    let mut indices = Vec::with_capacity(quad_model.indices_len + niko_model.indices_len);

    vertices.extend_from_slice(&QUAD_VERTICES);
    indices.extend_from_slice(&QUAD_INDICES);

    for vertex in obj.vertices {
      vertices.push(TexturedVertex {
        pos: vertex.position,
        normal: vertex.normal,
        tex_coords: [vertex.texture[0], vertex.texture[1]],
        ..Default::default()
      });
    }
    for index in obj.indices {
      indices.push(index + QUAD_INDICES.len() as u32);
    }

    let models = Models {
      quad: quad_model,
      niko: niko_model,
      total_index_count: indices.len() as u32,
    };

    Ok(Self {
      vertices,
      indices,
      models,
    })
  }

  pub fn vertices_size(&self) -> u64 {
    (self.vertices.len() * size_of::<TexturedVertex>()) as u64
  }

  pub fn indices_size(&self) -> u64 {
    (self.indices.len() * size_of::<u32>()) as u64
  }
}
