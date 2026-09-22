use std::{cmp::Ordering, io, path::PathBuf, sync::LazyLock};

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

fn unwrap_handle(handle: &Handle) -> (&PathBuf, u32) {
  match handle {
    Handle::Path { path, font_index } => (path, *font_index),
    Handle::Memory { .. } => panic!(),
  }
}

fn choose_font(fonts: &[Handle]) -> (&PathBuf, u32) {
  // choose font that contains "regular"
  for handle in fonts.iter() {
    let (path, font_index) = unwrap_handle(handle);

    let path_str = path
      .as_os_str()
      .to_str()
      .expect("Failed to font path str to str");
    if path_str.contains("REGULAR") || path_str.contains("egular") {
      return (path, font_index);
    }
  }

  // sort by path len first and by index after
  let min_handle = fonts.iter().min_by(|a, b| {
    let (path_a, index_a) = unwrap_handle(a);
    let (path_b, index_b) = unwrap_handle(b);

    match path_a.as_os_str().len().cmp(&path_b.as_os_str().len()) {
      Ordering::Equal => index_a.cmp(&index_b),
      other => other,
    }
  });

  unwrap_handle(min_handle.expect("No fonts found in family"))
}

// hopefully in the future there will be some centralized function that loads all required
// files at once
pub fn load_font() -> Result<FontBytes, FontError> {
  let source = SystemSource::new();
  let (family, family_name) = search_family(&source)?;

  log::debug!("Available fonts from chosen family:\n{:#?}", family.fonts());

  let (font_path, mut font_index) = choose_font(family.fonts());

  // not sure what index refers to in this case
  if !font_path.ends_with(".ttc") {
    font_index = 0;
  }

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
    font_index: font_index,
  })
}
