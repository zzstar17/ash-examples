use crate::{
  FERRIS_TEXTURE_OFFSET, FERRIS_TEXTURE_SIZE, KAKYOIN_TEXTURE_OFFSET, KAKYOIN_TEXTURE_SIZE,
  NIKO_TEXTURE_OFFSET, NIKO_TEXTURE_SIZE, SPRITES_TOTAL_SIZE, TEXTURE_PATH,
};

pub struct TextureData {
  pub bytes: Vec<u8>,
  pub width: u32,
  pub height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct TextureOffsets {
  pub ferris: TextureLoc,
  pub niko: TextureLoc,
  pub kakyoin: TextureLoc,
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
  }
}

impl TextureData {
  pub fn read_texture_bytes_as_rgba8() -> Result<Self, image::ImageError> {
    let img = image::ImageReader::open(TEXTURE_PATH)?
      .decode()?
      .into_rgba8();
    let width = img.width();
    let height = img.height();

    assert_eq!(
      [width, height],
      SPRITES_TOTAL_SIZE,
      "Invalid sprite dimensions"
    );

    let bytes = img.into_raw();
    assert!(bytes.len() == width as usize * height as usize * 4);
    Ok(Self {
      bytes,
      width,
      height,
    })
  }
}
