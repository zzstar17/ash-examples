use std::{
  marker::{PhantomData, PhantomPinned},
  ptr::{self, addr_of},
};

use ash::vk;

fn write_set<'a>(
  set: vk::DescriptorSet,
  binding: u32,
  descriptor_type: vk::DescriptorType,
) -> vk::WriteDescriptorSet<'a> {
  vk::WriteDescriptorSet {
    s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
    p_next: ptr::null(),
    dst_set: set,
    dst_binding: binding,
    dst_array_element: 0,
    descriptor_count: 1,
    descriptor_type,
    p_buffer_info: ptr::null(),
    p_image_info: ptr::null(),
    p_texel_buffer_view: ptr::null(),
    _marker: PhantomData,
  }
}

#[derive(Debug)]
pub struct BufferWriteDescriptorSet<'a> {
  inner: vk::WriteDescriptorSet<'a>,
  info: vk::DescriptorBufferInfo,
  _pin: PhantomPinned,
}

impl<'a> BufferWriteDescriptorSet<'a> {
  pub fn new(
    set: vk::DescriptorSet,
    binding: u32,
    descriptor_type: vk::DescriptorType,
    info: vk::DescriptorBufferInfo,
  ) -> Self {
    Self {
      inner: write_set(set, binding, descriptor_type),
      info,
      _pin: PhantomPinned,
    }
  }

  // returns a vk::WriteDescriptorSet that is valid for as long that self is not moved
  pub fn contextualize(&self) -> vk::WriteDescriptorSet<'_> {
    vk::WriteDescriptorSet {
      p_buffer_info: addr_of!(self.info),
      ..self.inner
    }
  }
}

pub fn storage_buffer_descriptor_set<'a>(
  set: vk::DescriptorSet,
  binding: u32,
  info: vk::DescriptorBufferInfo,
) -> BufferWriteDescriptorSet<'a> {
  BufferWriteDescriptorSet::new(set, binding, vk::DescriptorType::STORAGE_BUFFER, info)
}

#[derive(Debug)]
pub struct ImageWriteDescriptorSet<'a> {
  inner: vk::WriteDescriptorSet<'a>,
  info: vk::DescriptorImageInfo,
  _pin: PhantomPinned,
}

impl<'a> ImageWriteDescriptorSet<'a> {
  pub fn new(
    set: vk::DescriptorSet,
    binding: u32,
    descriptor_type: vk::DescriptorType,
    info: vk::DescriptorImageInfo,
  ) -> Self {
    Self {
      inner: write_set(set, binding, descriptor_type),
      info,
      _pin: PhantomPinned,
    }
  }

  // returns a vk::WriteDescriptorSet that is valid for as long that self is not moved
  pub fn contextualize(&self) -> vk::WriteDescriptorSet<'_> {
    vk::WriteDescriptorSet {
      p_image_info: addr_of!(self.info),
      ..self.inner
    }
  }
}

pub fn texture_write_descriptor_set<'a>(
  set: vk::DescriptorSet,
  texture_view: vk::ImageView,
  binding: u32,
) -> ImageWriteDescriptorSet<'a> {
  ImageWriteDescriptorSet::new(
    set,
    binding,
    vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
    vk::DescriptorImageInfo {
      sampler: vk::Sampler::null(),
      image_view: texture_view,
      image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    },
  )
}
