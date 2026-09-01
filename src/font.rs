use std::{io, path::PathBuf, sync::LazyLock};

use font_kit::{family_handle::FamilyHandle, handle::Handle, source::SystemSource};

use crate::TEXT_FONT;

// todo: most harfrust's structs are somewhat annoying references of references
// todo: maybe it's better to store all of this in a self referential struct?
pub static FONT_BYTES: LazyLock<FontBytes> =
  LazyLock::new(|| load_font().expect("Failed to load font"));
pub static FONT_REF: LazyLock<harfrust::FontRef> = LazyLock::new(|| {
  harfrust::FontRef::from_index(&FONT_BYTES.bytes, FONT_BYTES.font_index)
    .expect("Failed to read font data")
});
pub static SHAPER_DATA: LazyLock<harfrust::ShaperData> =
  LazyLock::new(|| harfrust::ShaperData::new(&FONT_REF));
pub static FONT_FACE: LazyLock<ttf_parser::Face> = LazyLock::new(|| {
  ttf_parser::Face::parse(&FONT_BYTES.bytes, FONT_BYTES.font_index)
    .expect("Failed to parse font face from font data")
});

pub struct FontBytes {
  pub bytes: Box<[u8]>,
  pub font_index: u32,
}

fn search_family(source: &SystemSource) -> Result<(FamilyHandle, &str), FontError> {
  for font_name in TEXT_FONT {
    match source.select_family_by_name(font_name) {
      Ok(handle) => return Ok((handle, font_name)),
      Err(_) => {
        continue;
      }
    }
  }
  Err(FontError::FontFamilyUnavailable)
}

#[derive(Debug, thiserror::Error)]
pub enum FontError {
  #[error("None of the specified font families are available on the system")]
  FontFamilyUnavailable,
  #[error("Failed to read font file ({1}): {0}")]
  SystemReadError(#[source] io::Error, PathBuf),
}

// hopefully in the future there will be some centralized function that loads all required
// files at once
pub fn load_font() -> Result<FontBytes, FontError> {
  let source = SystemSource::new();
  let (family, family_name) = search_family(&source)?;

  let (font_path, font_index) = &family
    .fonts()
    .iter()
    .find_map(|handle| match handle {
      Handle::Path { path, font_index } => {
        let path_str = path
          .as_os_str()
          .to_str()
          .expect("Failed to font path str to str");
        if path_str.contains("REGULAR") || path_str.contains("regular") {
          Some((path, *font_index))
        } else {
          None
        }
      }
      Handle::Memory { .. } => panic!(),
    })
    .unwrap_or_else(|| match &family.fonts()[0] {
      Handle::Path { path, font_index } => (path, *font_index),
      Handle::Memory { .. } => panic!(),
    });

  let font_bytes = std::fs::read(font_path)
    .map_err(|err| FontError::SystemReadError(err, (*font_path).clone()))?
    .into_boxed_slice();

  log::info!(
    "Loaded font \"{}\", index {} from {:?}",
    family_name,
    font_index,
    font_path
  );

  Ok(FontBytes {
    bytes: font_bytes,
    font_index: *font_index,
  })
}
