use std::{fs, io::Read};

use ash::vk;

use crate::{
  BLACK_TEXTURE_OFFSET, BLUE_TEXTURE_OFFSET, FERRIS_TEXTURE_OFFSET, FERRIS_TEXTURE_SIZE,
  GREEN_TEXTURE_OFFSET, KAKYOIN_TEXTURE_OFFSET, KAKYOIN_TEXTURE_SIZE, NIKO_TEXTURE_OFFSET,
  NIKO_TEXTURE_SIZE, RED_TEXTURE_OFFSET, SOLID_COLOR_TEXTURE_SIZE, SPRITES_TOTAL_SIZE,
  SPRITES_TOTAL_SIZE_F32, TEXTURE_PATH,
};

pub struct TextureData {
  pub reader: ktx2::Reader<Vec<u8>>,
  pub width: u32,
  pub height: u32,
  pub total_mip_levels_size: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum TextureLoadError {
  #[error("Failed to open or read files: {0}")]
  SystemIO(#[from] std::io::Error),

  #[error("Failed to parse textures file: {0}")]
  KtxParseError(#[from] ktx2::ParseError),
}

#[derive(Debug, Clone, Copy)]
pub struct TextureOffsets {
  pub ferris: TextureLoc,
  pub niko: TextureLoc,
  pub kakyoin: TextureLoc,
  pub black: TextureLoc,
  pub red: TextureLoc,
  pub green: TextureLoc,
  pub blue: TextureLoc,
}

#[derive(Debug, Clone, Copy)]
pub struct TextureLoc {
  // normalized
  pub offset: [f32; 2],
  pub size: [f32; 2],
}

impl TextureLoc {
  pub const fn from_render_extent(texture_size: [f32; 2], texture_offset: [f32; 2]) -> Self {
    let size = [
      texture_size[0] / SPRITES_TOTAL_SIZE_F32[0],
      texture_size[1] / SPRITES_TOTAL_SIZE_F32[1],
    ];
    let offset = [
      texture_offset[0] / SPRITES_TOTAL_SIZE_F32[0],
      texture_offset[1] / SPRITES_TOTAL_SIZE_F32[1],
    ];
    Self { offset, size }
  }
}

pub const fn get_texture_offsets() -> TextureOffsets {
  TextureOffsets {
    ferris: TextureLoc::from_render_extent(FERRIS_TEXTURE_SIZE, FERRIS_TEXTURE_OFFSET),
    niko: TextureLoc::from_render_extent(NIKO_TEXTURE_SIZE, NIKO_TEXTURE_OFFSET),
    kakyoin: TextureLoc::from_render_extent(KAKYOIN_TEXTURE_SIZE, KAKYOIN_TEXTURE_OFFSET),
    black: TextureLoc::from_render_extent(SOLID_COLOR_TEXTURE_SIZE, BLACK_TEXTURE_OFFSET),
    red: TextureLoc::from_render_extent(SOLID_COLOR_TEXTURE_SIZE, RED_TEXTURE_OFFSET),
    green: TextureLoc::from_render_extent(SOLID_COLOR_TEXTURE_SIZE, GREEN_TEXTURE_OFFSET),
    blue: TextureLoc::from_render_extent(SOLID_COLOR_TEXTURE_SIZE, BLUE_TEXTURE_OFFSET),
  }
}

impl TextureData {
  pub const TEXTURE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
  pub const TEXTURE_FORMAT_KTX2: ktx2::Format = ktx2::Format::R8G8B8A8_SRGB;

  pub fn read_texture_bytes_as_rgba8() -> Result<Self, TextureLoadError> {
    let mut file = fs::File::open(TEXTURE_PATH)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;

    let reader = ktx2::Reader::new(bytes)?;
    let header = reader.header();
    assert_eq!(header.format, Some(Self::TEXTURE_FORMAT_KTX2));

    log::debug!("Texture image header:\n    {:?}", header);
    let width = header.pixel_width;
    let height = header.pixel_height;
    assert_eq!(
      [width, height],
      SPRITES_TOTAL_SIZE,
      "Invalid sprite dimensions"
    );

    // assert ucompressed
    let first_level = reader.levels().next().unwrap();
    assert_eq!(
      first_level.data.len() as u64,
      first_level.uncompressed_byte_length
    );
    assert!(first_level.data.len() == width as usize * height as usize * 4);

    let total_mip_levels_size = reader.levels().map(|l| l.uncompressed_byte_length).sum();

    Ok(Self {
      reader,
      width,
      height,
      total_mip_levels_size,
    })
  }
}
