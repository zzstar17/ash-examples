use std::{fs::File, io::BufReader};

use crate::{render::TexturedVertex, KAKYOIN_MODEL_PATH, NIKO_MODEL_PATH};

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

pub const QUAD_INDICES: [u32; 6] = [0, 1, 2, 3, 2, 1];

pub struct LoadedModels {
  pub vertices: Vec<TexturedVertex>,
  pub indices: Vec<u32>,

  pub models: Models,
}

#[derive(Debug, Clone, Copy)]
pub struct Models {
  pub quad: ModelOffset,
  pub niko: ModelOffset,
  pub kakyoin: ModelOffset,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelOffset {
  pub vertices_offset: usize,
  pub indices_offset: usize,
  pub vertices_len: usize,
  pub indices_len: usize,
}

impl LoadedModels {
  pub fn load() -> Result<Self, (obj::ObjError, &'static str)> {
    let niko_obj: obj::Obj<obj::TexturedVertex, u32> = {
      let niko_f =
        File::open(NIKO_MODEL_PATH).map_err(|err| (obj::ObjError::from(err), NIKO_MODEL_PATH))?;
      let mut buff_reader = BufReader::new(niko_f);
      obj::load_obj(&mut buff_reader).map_err(|err| (err, NIKO_MODEL_PATH))?
    };

    let kakyoin_obj: obj::Obj<obj::TexturedVertex, u32> = {
      let niko_f = File::open(KAKYOIN_MODEL_PATH)
        .map_err(|err| (obj::ObjError::from(err), KAKYOIN_MODEL_PATH))?;
      let mut buff_reader = BufReader::new(niko_f);
      obj::load_obj(&mut buff_reader).map_err(|err| (err, KAKYOIN_MODEL_PATH))?
    };

    let quad_model = ModelOffset {
      vertices_offset: 0,
      indices_offset: 0,
      vertices_len: QUAD_VERTICES.len(),
      indices_len: QUAD_INDICES.len(),
    };

    let niko_model = ModelOffset {
      vertices_offset: quad_model.vertices_len,
      indices_offset: quad_model.indices_len,
      vertices_len: niko_obj.vertices.len(),
      indices_len: niko_obj.indices.len(),
    };

    let kakyoin_model = ModelOffset {
      vertices_offset: niko_model.vertices_offset + niko_model.vertices_len,
      indices_offset: niko_model.indices_offset + niko_model.indices_len,
      vertices_len: kakyoin_obj.vertices.len(),
      indices_len: kakyoin_obj.indices.len(),
    };

    let mut vertices = Vec::with_capacity(
      quad_model.vertices_len + niko_model.vertices_len + kakyoin_model.vertices_len,
    );
    let mut indices = Vec::with_capacity(
      quad_model.indices_len + niko_model.indices_len + kakyoin_model.indices_len,
    );

    vertices.extend_from_slice(&QUAD_VERTICES);
    indices.extend_from_slice(&QUAD_INDICES);

    // flip y coordinates
    for vertex in niko_obj.vertices.into_iter().chain(kakyoin_obj.vertices) {
      vertices.push(TexturedVertex {
        pos: [
          vertex.position[0],
          1.0 - vertex.position[1],
          vertex.position[2],
        ],
        normal: [vertex.normal[0], 1.0 - vertex.normal[1], vertex.normal[2]],
        tex_coords: [vertex.texture[0], 1.0 - vertex.texture[1]],
        ..Default::default()
      });
    }
    indices.extend_from_slice(&niko_obj.indices);
    indices.extend_from_slice(&kakyoin_obj.indices);

    let models = Models {
      quad: quad_model,
      niko: niko_model,
      kakyoin: kakyoin_model,
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
