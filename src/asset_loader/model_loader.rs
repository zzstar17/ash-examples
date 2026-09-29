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
  // top right
  TexturedVertex {
    pos: [1.0, -1.0, 0.0],
    normal: [0.0, 0.0, 1.0],
    tex_coords: [1.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom left
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

pub const CUBE_VERTICES: [TexturedVertex; 24] = [
  // FRONT FACE
  // top left
  TexturedVertex {
    pos: [-1.0, -1.0, 1.0],
    normal: [0.0, 0.0, 1.0],
    tex_coords: [0.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // top right
  TexturedVertex {
    pos: [1.0, -1.0, 1.0],
    normal: [0.0, 0.0, 1.0],
    tex_coords: [1.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom left
  TexturedVertex {
    pos: [-1.0, 1.0, 1.0],
    normal: [0.0, 0.0, 1.0],
    tex_coords: [0.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom right
  TexturedVertex {
    pos: [1.0, 1.0, 1.0],
    normal: [0.0, 0.0, 1.0],
    tex_coords: [1.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // LEFT FACE
  // top left
  TexturedVertex {
    pos: [-1.0, -1.0, -1.0],
    normal: [-1.0, 0.0, 0.0],
    tex_coords: [0.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // top right
  TexturedVertex {
    pos: [-1.0, -1.0, 1.0],
    normal: [-1.0, 0.0, 0.0],
    tex_coords: [1.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom left
  TexturedVertex {
    pos: [-1.0, 1.0, -1.0],
    normal: [-1.0, 0.0, 0.0],
    tex_coords: [0.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom right
  TexturedVertex {
    pos: [-1.0, 1.0, 1.0],
    normal: [-1.0, 0.0, 0.0],
    tex_coords: [1.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // BACK FACE
  // top left
  TexturedVertex {
    pos: [1.0, -1.0, -1.0],
    normal: [0.0, 0.0, -1.0],
    tex_coords: [0.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // top right
  TexturedVertex {
    pos: [-1.0, -1.0, -1.0],
    normal: [0.0, 0.0, -1.0],
    tex_coords: [1.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom left
  TexturedVertex {
    pos: [1.0, 1.0, -1.0],
    normal: [0.0, 0.0, -1.0],
    tex_coords: [0.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom right
  TexturedVertex {
    pos: [-1.0, 1.0, -1.0],
    normal: [0.0, 0.0, -1.0],
    tex_coords: [1.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // RIGHT FACE
  // top left
  TexturedVertex {
    pos: [1.0, -1.0, 1.0],
    normal: [1.0, 0.0, 0.0],
    tex_coords: [0.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // top right
  TexturedVertex {
    pos: [1.0, -1.0, -1.0],
    normal: [1.0, 0.0, 0.0],
    tex_coords: [1.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom left
  TexturedVertex {
    pos: [1.0, 1.0, 1.0],
    normal: [1.0, 0.0, 0.0],
    tex_coords: [0.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom right
  TexturedVertex {
    pos: [1.0, 1.0, -1.0],
    normal: [1.0, 0.0, 0.0],
    tex_coords: [1.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // TOP FACE
  // top left
  TexturedVertex {
    pos: [-1.0, -1.0, -1.0],
    normal: [0.0, -1.0, 0.0],
    tex_coords: [0.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // top right
  TexturedVertex {
    pos: [1.0, -1.0, -1.0],
    normal: [0.0, -1.0, 0.0],
    tex_coords: [1.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom left
  TexturedVertex {
    pos: [-1.0, -1.0, 1.0],
    normal: [0.0, -1.0, 0.0],
    tex_coords: [0.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom right
  TexturedVertex {
    pos: [1.0, -1.0, 1.0],
    normal: [0.0, -1.0, 0.0],
    tex_coords: [1.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // BOTTOM FACE
  // top left
  TexturedVertex {
    pos: [-1.0, 1.0, 1.0],
    normal: [0.0, 1.0, 0.0],
    tex_coords: [0.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // top right
  TexturedVertex {
    pos: [1.0, 1.0, 1.0],
    normal: [0.0, 1.0, 0.0],
    tex_coords: [1.0, 0.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom left
  TexturedVertex {
    pos: [-1.0, 1.0, -1.0],
    normal: [0.0, 1.0, 0.0],
    tex_coords: [0.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
  // bottom right
  TexturedVertex {
    pos: [1.0, 1.0, -1.0],
    normal: [0.0, 1.0, 0.0],
    tex_coords: [1.0, 1.0],
    _padding0: 0.0,
    _padding1: 0.0,
  },
];

pub const CUBE_INDICES: [u32; 36] = [
  0, 1, 2, 3, 2, 1, // front
  4, 5, 6, 7, 6, 5, // left
  8, 9, 10, 11, 10, 9, // back
  12, 13, 14, 15, 14, 13, // right
  16, 17, 18, 19, 18, 17, // right
  20, 21, 22, 23, 22, 21, // right
];

pub struct LoadedModels {
  pub vertices: Vec<TexturedVertex>,
  pub indices: Vec<u32>,

  pub models: Models,
}

#[derive(Debug, Clone, Copy)]
pub struct Models {
  pub quad: ModelOffset,
  pub cube: ModelOffset,
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
    let mut niko_obj: obj::Obj<obj::TexturedVertex, u32> = {
      let niko_f =
        File::open(NIKO_MODEL_PATH).map_err(|err| (obj::ObjError::from(err), NIKO_MODEL_PATH))?;
      let mut buff_reader = BufReader::new(niko_f);
      obj::load_obj(&mut buff_reader).map_err(|err| (err, NIKO_MODEL_PATH))?
    };

    // todo: Fix niko model's vertices's face orientation
    // temporary fix to flip triangles orientation counter-clockwise
    assert!(niko_obj.indices.len().is_multiple_of(3));
    for triangle in niko_obj.indices.chunks_exact_mut(3) {
      let temp = triangle[0];
      triangle[0] = triangle[2];
      triangle[2] = temp;
    }

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
    let cube_model = ModelOffset {
      vertices_offset: quad_model.vertices_len,
      indices_offset: quad_model.indices_len,
      vertices_len: CUBE_VERTICES.len(),
      indices_len: CUBE_INDICES.len(),
    };

    let niko_model = ModelOffset {
      vertices_offset: cube_model.vertices_offset + cube_model.vertices_len,
      indices_offset: cube_model.indices_offset + cube_model.indices_len,
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
    vertices.extend_from_slice(&CUBE_VERTICES);
    indices.extend_from_slice(&CUBE_INDICES);

    // flip y coordinates
    for vertex in niko_obj.vertices.into_iter().chain(kakyoin_obj.vertices) {
      vertices.push(TexturedVertex {
        pos: [
          vertex.position[0],
          // vertex.position[1],
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
      cube: cube_model,
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
