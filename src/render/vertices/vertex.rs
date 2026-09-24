use std::mem::{offset_of, size_of};

use ash::vk;

#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Vertex {
  pub pos: [f32; 2],
  pub tex_coords: [f32; 2],
}

// std430 layout
#[repr(C)]
#[derive(Default, Copy, PartialEq, Clone, Debug)]
pub struct TexturedVertex {
  pub pos: [f32; 3],
  pub _padding0: f32,
  pub normal: [f32; 3],
  pub _padding1: f32,
  /// texture coordinates
  pub tex_coords: [f32; 2],
}

impl TexturedVertex {
  const ATTRIBUTE_SIZE: usize = 3;

  pub const fn get_binding_description(binding: u32) -> vk::VertexInputBindingDescription {
    vk::VertexInputBindingDescription {
      binding,
      stride: size_of::<Self>() as u32,
      input_rate: vk::VertexInputRate::VERTEX,
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
        format: vk::Format::R32G32B32_SFLOAT,
        offset: offset_of!(Self, pos) as u32,
      },
      vk::VertexInputAttributeDescription {
        location: offset + 1,
        binding,
        format: vk::Format::R32G32B32_SFLOAT,
        offset: offset_of!(Self, normal) as u32,
      },
      vk::VertexInputAttributeDescription {
        location: offset + 2,
        binding,
        format: vk::Format::R32G32_SFLOAT,
        offset: offset_of!(Self, tex_coords) as u32,
      },
    ]
  }
}
