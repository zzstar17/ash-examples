use std::mem::offset_of;

use ash::vk;

// size and alignment: 4
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Particle {
  pub pos: [f32; 2],
  pub vel: [f32; 2],
}

// impl instance vertex for Particle
impl Particle {
  const ATTRIBUTE_SIZE: usize = 2;

  pub const fn get_binding_description(binding: u32) -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription {
      binding,
      stride: size_of::<Self>() as u32,
      input_rate: vk::VertexInputRate::INSTANCE,
    }
  }

  pub const fn get_attribute_descriptions(
    offset: u32,
    binding: u32,
  ) -> [vk::VertexInputAttributeDescription; Self::ATTRIBUTE_SIZE] {
    [
      vk::VertexInputAttributeDescription {
        location: offset,
        binding,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(Self, pos) as u32,
      },
      vk::VertexInputAttributeDescription {
        location: offset + 1,
        binding,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(Self, vel) as u32,
      },
    ]
  }
}
