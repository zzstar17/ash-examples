use std::{fs, io::Read};

use ash::vk;

use crate::{
  BLACK_TEXTURE_OFFSET, BLUE_TEXTURE_OFFSET, FERRIS_TEXTURE_OFFSET, FERRIS_TEXTURE_SIZE,
  GREEN_TEXTURE_OFFSET, KAKYOIN_TEXTURE_OFFSET, KAKYOIN_TEXTURE_SIZE, NIKO_TEXTURE_OFFSET,
  NIKO_TEXTURE_SIZE, RED_TEXTURE_OFFSET, SOLID_COLOR_TEXTURE_SIZE, SPRITES_TOTAL_SIZE,
  TEXTURE_PATH,
};

pub const TEXTURE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
pub const TEXTURE_FORMAT_KTX2: ktx2::Format = ktx2::Format::R8G8B8A8_SRGB;

pub struct TextureData {
  pub reader: ktx2::Reader<Vec<u8>>,
  pub width: u32,
  pub height: u32,
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
  // in pixels
  pub offset: [f32; 2],
  pub size: [f32; 2],
}

pub const fn get_texture_offsets() -> TextureOffsets {
  TextureOffsets {
    ferris: TextureLoc {
      offset: FERRIS_TEXTURE_OFFSET,
      size: FERRIS_TEXTURE_SIZE,
    },
    niko: TextureLoc {
      offset: NIKO_TEXTURE_OFFSET,
      size: NIKO_TEXTURE_SIZE,
    },
    kakyoin: TextureLoc {
      offset: KAKYOIN_TEXTURE_OFFSET,
      size: KAKYOIN_TEXTURE_SIZE,
    },
    black: TextureLoc {
      offset: BLACK_TEXTURE_OFFSET,
      size: SOLID_COLOR_TEXTURE_SIZE,
    },
    red: TextureLoc {
      offset: RED_TEXTURE_OFFSET,
      size: SOLID_COLOR_TEXTURE_SIZE,
    },
    green: TextureLoc {
      offset: GREEN_TEXTURE_OFFSET,
      size: SOLID_COLOR_TEXTURE_SIZE,
    },
    blue: TextureLoc {
      offset: BLUE_TEXTURE_OFFSET,
      size: SOLID_COLOR_TEXTURE_SIZE,
    },
  }
}

impl TextureData {
  pub fn read_texture_bytes_as_rgba8() -> Result<Self, TextureLoadError> {
    let mut file = fs::File::open(TEXTURE_PATH)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;

    let reader = ktx2::Reader::new(bytes)?;
    let header = reader.header();
    assert_eq!(header.format, Some(TEXTURE_FORMAT_KTX2));

    log::debug!("Texture image header:\n    {:?}", header);
    let width = header.pixel_width;
    let height = header.pixel_height;
    assert_eq!(
      [width, height],
      SPRITES_TOTAL_SIZE,
      "Invalid sprite dimensions"
    );

    // assert ucompressed
    for level in reader.levels() {
      assert_eq!(level.data.len() as u64, level.uncompressed_byte_length);
      assert!(level.data.len() == width as usize * height as usize * 4);
    }

    Ok(Self {
      reader,
      width,
      height,
    })
  }

  pub fn bytes(&self) -> &[u8] {
    let level = self.reader.levels().next().unwrap();
    level.data
  }
}
