use std::{
  fs::{self},
  io::Read,
  path::Path,
  time::Instant,
};

use gltf::{
  accessor::{DataType, Dimensions},
  buffer::Source as BufferSource,
  image::Source as ImageSource,
  Gltf, Semantic,
};
use itertools::Itertools;
use relative_path::RelativePath;

use crate::{
  asset_loader::texture_loader::{ImageLoadError, LoadedImageData},
  render::TexturedVertex,
};

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

pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 3, 2, 1];

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

pub const CUBE_INDICES: [u16; 36] = [
  0, 1, 2, 3, 2, 1, // front
  4, 5, 6, 7, 6, 5, // left
  8, 9, 10, 11, 10, 9, // back
  12, 13, 14, 15, 14, 13, // right
  16, 17, 18, 19, 18, 17, // right
  20, 21, 22, 23, 22, 21, // right
];

pub struct LoadedModels {
  pub gltf: Gltf,

  pub meshes: Vec<Mesh>,
  pub image_sizes: Vec<ImageSize>,
  pub model_gpu_data: ModelGpuData,

  pub quad_mesh_index: usize,
  pub cube_mesh_index: usize,
}

pub struct AssetLocationData {
  pub gltf: Gltf,

  pub meshes: Vec<Mesh>,
  pub image_sizes: Vec<ImageSize>,

  pub quad_mesh_index: usize,
  pub cube_mesh_index: usize,
}

pub struct ModelGpuData {
  pub vertices: Vec<TexturedVertex>,
  pub indices: Vec<u16>,
  pub images: Vec<LoadedImageData>,
}

#[derive(Debug, Clone, Copy)]
pub struct Mesh {
  pub vertices_offset: usize,
  pub vertices_count: usize,
  pub indices_offset: usize,
  pub indices_count: usize,
  pub image_index: Option<usize>,
  pub semi_transparent: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ImageSize {
  pub width: u32,
  pub height: u32,
}

fn print_tree(node: &gltf::Node, depth: i32) {
  for _ in 0..(depth - 1) {
    print!("  ");
  }
  print!(" -");
  print!(" Node {}", node.index());
  print!(" ({})", node.name().unwrap_or("<Unnamed>"));
  println!();

  for child in node.children() {
    print_tree(&child, depth + 1);
  }
}

#[derive(Debug, thiserror::Error)]
pub enum ModelLoadError {
  #[error("Failed to open or read files: {0}")]
  SystemIO(#[from] std::io::Error),

  #[error("Failed to parse glTF model file")]
  GLTFError(#[from] gltf::Error),

  #[error("Failed to load image:\n{0}")]
  ImageLoadError(#[from] ImageLoadError),
}

struct BufferSlice<'a> {
  pub bytes: &'a [u8],
  pub offset: usize,
  pub count: usize,
  pub data_type: DataType,
  pub dimensions: Dimensions,
}

fn read_buffer_with_accessor<'a>(
  buffer_bytes: &'a [Vec<u8>],
  accessor: gltf::Accessor,
) -> BufferSlice<'a> {
  let accessor_offset = accessor.offset();
  match accessor.view() {
    Some(view) => {
      let buffer_index = view.buffer().index();
      let view_size = view.length();
      let view_offset = view.offset();
      assert!(view.stride().is_none());

      let start_i = accessor_offset + view_offset;
      let bytes = &buffer_bytes[buffer_index][start_i..(start_i + view_size)];

      BufferSlice {
        bytes,
        offset: start_i,
        count: accessor.count(),
        data_type: accessor.data_type(),
        dimensions: accessor.dimensions(),
      }
    }
    None => unimplemented!(),
  }
}

unsafe fn read_xyz(slice: &[u8; 12]) -> [f32; 3] {
  (slice.as_ptr() as *const [f32; 3]).read()
}

unsafe fn read_xy(slice: &[u8; 8]) -> [f32; 2] {
  (slice.as_ptr() as *const [f32; 2]).read()
}

fn append_vertices(
  vertices: &mut Vec<TexturedVertex>,
  positions_slice: BufferSlice,
  normals_slice: BufferSlice,
  texture_coors_slice: BufferSlice,
) {
  assert_eq!(positions_slice.data_type, DataType::F32);
  assert_eq!(normals_slice.data_type, DataType::F32);
  assert_eq!(texture_coors_slice.data_type, DataType::F32);
  assert_eq!(positions_slice.dimensions, Dimensions::Vec3);
  assert_eq!(normals_slice.dimensions, Dimensions::Vec3);
  assert_eq!(texture_coors_slice.dimensions, Dimensions::Vec2);

  assert!(positions_slice.bytes.len().is_multiple_of(12));
  assert!(normals_slice.bytes.len().is_multiple_of(12));
  assert!(texture_coors_slice.bytes.len().is_multiple_of(8));

  let positions_iter = positions_slice
    .bytes
    .as_chunks::<12>()
    .0
    .iter()
    .map(|subslice| unsafe { read_xyz(subslice) });
  let normals_iter = normals_slice
    .bytes
    .as_chunks::<12>()
    .0
    .iter()
    .map(|subslice| unsafe { read_xyz(subslice) });
  let texture_coors_iter = texture_coors_slice
    .bytes
    .as_chunks::<8>()
    .0
    .iter()
    .map(|subslice| unsafe { read_xy(subslice) });

  vertices.reserve(positions_slice.count);
  for ((position, normal), texture) in positions_iter
    .zip_eq(normals_iter)
    .zip_eq(texture_coors_iter)
  {
    vertices.push(TexturedVertex {
      pos: [
        position[0],
        // vertex.position[1],
        position[1],
        position[2],
      ],
      normal: [normal[0], normal[1], normal[2]],
      tex_coords: [texture[0], texture[1]],
      ..Default::default()
    });
  }
}

fn append_indices(indices: &mut Vec<u16>, indices_slice: BufferSlice) {
  assert_eq!(indices_slice.data_type, DataType::U16);

  assert!(indices_slice.bytes.len().is_multiple_of(2));

  let indices_iter = indices_slice
    .bytes
    .as_chunks::<2>()
    .0
    .iter()
    .map(|subslice| u16::from_ne_bytes(*subslice));

  indices.reserve(indices_slice.count);
  for index in indices_iter {
    indices.push(index);
  }
}

impl LoadedModels {
  pub fn load() -> Result<Self, ModelLoadError> {
    let scene_path = Path::new("./assets/models/Niko_xinus22.gltf");
    let mut scene_dir = scene_path.to_path_buf();
    scene_dir.pop();

    let file = std::fs::File::open(&scene_path)?;
    let reader = std::io::BufReader::new(file);
    let gltf = gltf::Gltf::from_reader(reader)?;

    let scene: gltf::Scene<'_> = gltf.scenes().next().unwrap();

    log::info!("Loading scene \"{}\"", scene.name().unwrap_or("<Unnamed>"));
    for node in scene.nodes() {
      print_tree(&node, 1);
    }

    let mut buffers = Vec::with_capacity(gltf.buffers().len());
    for buffer_ref in gltf.buffers() {
      let buffer_data = match buffer_ref.source() {
        BufferSource::Bin => {
          unimplemented!()
        }
        BufferSource::Uri(uri) => {
          let buffer_path = RelativePath::new(uri).to_logical_path(&scene_dir);
          log::info!(
            "Loading model buffer \"{}\" at {:?}",
            buffer_ref.name().unwrap_or("<Unnamed>"),
            buffer_path
          );

          let now = Instant::now();
          let mut file = fs::File::open(&buffer_path)?;
          let mut bytes = Vec::new();
          file.read_to_end(&mut bytes)?;

          assert_eq!(bytes.len(), buffer_ref.length());
          log::info!(
            "Loaded model buffer \"{}\" at {:?} in {}μs",
            buffer_ref.name().unwrap_or("<Unnamed>"),
            buffer_path,
            now.elapsed().as_micros()
          );

          bytes
        }
      };
      buffers.push(buffer_data);
    }

    let mut vertices = Vec::with_capacity(QUAD_VERTICES.len() + CUBE_VERTICES.len());
    let mut indices = Vec::with_capacity(QUAD_INDICES.len() + CUBE_INDICES.len());
    let mut meshes = Vec::with_capacity(gltf.meshes().len() + 2);

    vertices.extend_from_slice(&QUAD_VERTICES);
    indices.extend_from_slice(&QUAD_INDICES);
    let quad_mesh_index = meshes.len();
    meshes.push(Mesh {
      vertices_offset: vertices.len(),
      vertices_count: QUAD_VERTICES.len(),
      indices_offset: indices.len(),
      indices_count: QUAD_INDICES.len(),
      image_index: None,
      semi_transparent: false,
    });

    vertices.extend_from_slice(&CUBE_VERTICES);
    indices.extend_from_slice(&CUBE_INDICES);
    let cube_mesh_index = meshes.len();
    meshes.push(Mesh {
      vertices_offset: vertices.len(),
      vertices_count: CUBE_VERTICES.len(),
      indices_offset: indices.len(),
      indices_count: CUBE_INDICES.len(),
      image_index: None,
      semi_transparent: false,
    });

    for mesh_ref in gltf.meshes() {
      let primitive = mesh_ref.primitives().next().unwrap();
      let mut positions = None;
      let mut normals = None;
      let mut texture_coors = None;
      for (semantic, accessor) in primitive.attributes() {
        match semantic {
          Semantic::Positions => {
            positions = Some(read_buffer_with_accessor(buffers.as_slice(), accessor))
          }
          Semantic::Normals => {
            normals = Some(read_buffer_with_accessor(buffers.as_slice(), accessor))
          }
          Semantic::TexCoords(_) => {
            assert!(texture_coors.is_none());
            texture_coors = Some(read_buffer_with_accessor(buffers.as_slice(), accessor));
          }
          _ => {}
        }
      }

      let positions = positions.unwrap();
      let normals = normals.unwrap();
      if texture_coors.is_none() {
        continue;
      }
      let texture_coors = texture_coors.unwrap();

      let indices_slice =
        read_buffer_with_accessor(buffers.as_slice(), primitive.indices().unwrap());

      let material = primitive.material();
      let texture = material
        .pbr_metallic_roughness()
        .base_color_texture()
        .unwrap()
        .texture();
      let image_index = texture.source().index();

      let vertices_offset = vertices.len();
      let indices_offset = indices.len();
      let vertices_count = positions.count;
      let indices_count = indices_slice.count;

      log::debug!(
        "Adding mesh \"{}\", material \"{}\" (image \"{}\") with {} vertices and {} indices",
        mesh_ref.name().unwrap_or("<Unnamed>"),
        material.name().unwrap_or("<Unnamed>"),
        texture.source().name().unwrap_or("<Unnamed>"),
        positions.count,
        indices_slice.count
      );
      append_vertices(&mut vertices, positions, normals, texture_coors);
      append_indices(&mut indices, indices_slice);

      // todo: add a better way of doing this
      let semi_transparent = match mesh_ref.name() {
        Some(name) => name.ends_with("Glass"),
        None => false,
      };

      meshes.push(Mesh {
        vertices_offset,
        vertices_count,
        indices_offset,
        indices_count,
        image_index: Some(image_index),
        semi_transparent,
      })
    }

    let mut images = Vec::with_capacity(gltf.images().len());
    let mut image_sizes = Vec::with_capacity(gltf.images().len());
    for image_ref in gltf.images() {
      let image_data = match image_ref.source() {
        ImageSource::View { .. } => {
          unimplemented!()
        }
        ImageSource::Uri { uri, mime_type } => {
          let image_path = RelativePath::new(uri).to_logical_path(&scene_dir);
          log::info!(
            "Loading image \"{}\" at {:?}",
            image_ref.name().unwrap_or("<Unnamed>"),
            image_path
          );

          let now = Instant::now();
          let mime_type = mime_type.expect("Texture missing mimetype");
          let data = match mime_type {
            "image/ktx2" => LoadedImageData::read_texture_bytes_as_rgba8(&image_path),
            _ => unimplemented!(),
          }?;
          log::info!(
            "Loaded image \"{}\" at {:?} in {}μs",
            image_ref.name().unwrap_or("<Unnamed>"),
            image_path,
            now.elapsed().as_micros()
          );

          data
        }
      };
      let image_size = ImageSize {
        width: image_data.width,
        height: image_data.height,
      };
      images.push(image_data);
      image_sizes.push(image_size);
    }

    Ok(Self {
      gltf,
      model_gpu_data: ModelGpuData {
        vertices,
        indices,
        images,
      },
      meshes,
      image_sizes,
      quad_mesh_index,
      cube_mesh_index,
    })
  }

  pub fn into_separate(self) -> (ModelGpuData, AssetLocationData) {
    (
      self.model_gpu_data,
      AssetLocationData {
        gltf: self.gltf,
        meshes: self.meshes,
        image_sizes: self.image_sizes,
        quad_mesh_index: self.quad_mesh_index,
        cube_mesh_index: self.cube_mesh_index,
      },
    )
  }
}

impl ModelGpuData {
  pub fn vertices_size(&self) -> u64 {
    (self.vertices.len() * size_of::<TexturedVertex>()) as u64
  }

  pub fn indices_size(&self) -> u64 {
    (self.indices.len() * size_of::<u32>()) as u64
  }
}
