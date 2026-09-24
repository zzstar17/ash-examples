mod model_loader;
mod shader_loader;

pub use model_loader::{LoadedModels, ModelOffset, Models, QUAD_INDICES};
pub use shader_loader::{ShaderLoadError, ShaderLoader};

use crate::TEXTURE_PATH;

pub struct SpriteTextureData {
  pub bytes: Vec<u8>,
  pub width: u32,
  pub height: u32,
}

impl SpriteTextureData {
  pub fn read_texture_bytes_as_rgba8() -> Result<Self, image::ImageError> {
    let img = image::ImageReader::open(TEXTURE_PATH)?
      .decode()?
      .into_rgba8();
    let width = img.width();
    let height = img.height();

    let bytes = img.into_raw();
    assert!(bytes.len() == width as usize * height as usize * 4);
    Ok(Self {
      bytes,
      width,
      height,
    })
  }
}
