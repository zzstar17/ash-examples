use ash::vk;

pub const KNOWN_FORMATS: [vk::Format; 4] = [
  vk::Format::R8G8B8A8_SRGB,
  vk::Format::B8G8R8A8_SRGB,
  vk::Format::R8G8B8A8_UNORM,
  vk::Format::B8G8R8A8_UNORM,
];

fn convert_rgba_to_bgra(bytes: &mut [u8]) {
  let (chunks, remainder) = bytes.as_chunks_mut::<4>();
  assert!(remainder.is_empty());
  for pixel in chunks {
    pixel.swap(0, 2); // swap B and R
  }
}

pub fn convert_rgba_data_to_format(data: &mut [u8], target_format: vk::Format) {
  match target_format {
    vk::Format::R8G8B8A8_SRGB | vk::Format::R8G8B8A8_UNORM => {}
    vk::Format::B8G8R8A8_SRGB | vk::Format::B8G8R8A8_UNORM => {
      convert_rgba_to_bgra(data);
    }
    _ => panic!("Trying to convert to unsupported format"),
  }
}
