use std::{fs, io::Read};

use ash::vk;

pub struct LoadedImageData {
  pub reader: ktx2::Reader<Vec<u8>>,
  pub width: u32,
  pub height: u32,
  pub total_mip_levels_size: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ImageLoadError {
  #[error("Failed to open or read files: {0}")]
  SystemIO(#[from] std::io::Error),

  #[error("Failed to parse textures file: {0}")]
  KtxParseError(#[from] ktx2::ParseError),
}

impl LoadedImageData {
  pub const TEXTURE_FORMAT: vk::Format = vk::Format::R8G8B8A8_SRGB;
  pub const TEXTURE_FORMAT_KTX2: ktx2::Format = ktx2::Format::R8G8B8A8_SRGB;

  pub fn read_texture_bytes_as_rgba8<P>(path: P) -> Result<Self, ImageLoadError>
  where
    P: AsRef<std::path::Path>,
  {
    let mut file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;

    let reader = ktx2::Reader::new(bytes)?;
    let header = reader.header();
    assert_eq!(header.format, Some(Self::TEXTURE_FORMAT_KTX2));

    log::debug!("Texture image header:\n    {:?}", header);
    let width = header.pixel_width;
    let height = header.pixel_height;

    // assert uncompressed
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
