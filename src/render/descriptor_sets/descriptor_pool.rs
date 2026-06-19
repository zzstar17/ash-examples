use std::{marker::PhantomData, ptr};

use ash::vk;
use vkobjects::{errors::OutOfMemoryError, DeviceManuallyDestroyed};

use super::texture_write_descriptor_set;

fn create_texture_sampler(device: &ash::Device) -> Result<vk::Sampler, OutOfMemoryError> {
  let sampler_create_info = vk::SamplerCreateInfo {
    s_type: vk::StructureType::SAMPLER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::SamplerCreateFlags::empty(),
    mag_filter: vk::Filter::LINEAR,
    min_filter: vk::Filter::LINEAR,
    address_mode_u: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    address_mode_v: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    address_mode_w: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    anisotropy_enable: vk::FALSE,
    max_anisotropy: 0.0,
    border_color: vk::BorderColor::INT_TRANSPARENT_BLACK,
    unnormalized_coordinates: vk::TRUE,
    compare_enable: vk::FALSE,
    compare_op: vk::CompareOp::NEVER,
    mipmap_mode: vk::SamplerMipmapMode::NEAREST,
    mip_lod_bias: 0.0,
    max_lod: 0.0,
    min_lod: 0.0,
    _marker: PhantomData,
  };
  unsafe { device.create_sampler(&sampler_create_info, None) }.map_err(|err| err.into())
}

pub struct DescriptorPool {
  pub texture_layout: vk::DescriptorSetLayout,
  texture_sampler: vk::Sampler,
  pub texture_set: vk::DescriptorSet,

  pool: vk::DescriptorPool,
}

impl DescriptorPool {
  // this pool only allocates one set and does not reallocate
  const SET_COUNT: u32 = Self::SIZES[0].descriptor_count;

  const SIZES: [vk::DescriptorPoolSize; 1] = [vk::DescriptorPoolSize {
    ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
    descriptor_count: 1,
  }];

  fn graphics_layout_bindings<'a>(
    texture_sampler: *const vk::Sampler,
  ) -> [vk::DescriptorSetLayoutBinding<'a>; 1] {
    [vk::DescriptorSetLayoutBinding {
      binding: 0,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::FRAGMENT,
      p_immutable_samplers: texture_sampler,
      _marker: PhantomData,
    }]
  }

  pub fn new(device: &ash::Device, texture_view: vk::ImageView) -> Result<Self, OutOfMemoryError> {
    let texture_sampler = create_texture_sampler(device)?;

    let texture_layout = Self::create_graphics_layout(device, texture_sampler)?;

    let pool = {
      let pool_create_info = vk::DescriptorPoolCreateInfo {
        s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
        p_next: ptr::null(),
        pool_size_count: Self::SIZES.len() as u32,
        p_pool_sizes: Self::SIZES.as_ptr(),
        max_sets: Self::SET_COUNT,
        flags: vk::DescriptorPoolCreateFlags::empty(),
        _marker: PhantomData,
      };
      unsafe { device.create_descriptor_pool(&pool_create_info, None) }
    }?;

    let set = allocate_sets(device, pool, &[texture_layout])?[0];
    let write = texture_write_descriptor_set(set, texture_view, 0); // same binding as in layout
                                                                    // update texture set
    unsafe {
      let contextualized = write.contextualize();
      device.update_descriptor_sets(&[contextualized], &[]);
    }

    Ok(Self {
      texture_sampler,
      texture_layout,
      texture_set: set,
      pool,
    })
  }

  fn create_graphics_layout(
    device: &ash::Device,
    texture_sampler: vk::Sampler,
  ) -> Result<vk::DescriptorSetLayout, OutOfMemoryError> {
    let ptr = &texture_sampler;
    let bindings = Self::graphics_layout_bindings(ptr);
    create_layout(device, &bindings)
  }
}

impl DeviceManuallyDestroyed for DescriptorPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_descriptor_pool(self.pool, None);

    device.destroy_descriptor_set_layout(self.texture_layout, None);
    device.destroy_sampler(self.texture_sampler, None);
  }
}

fn create_layout(
  device: &ash::Device,
  bindings: &[vk::DescriptorSetLayoutBinding],
) -> Result<vk::DescriptorSetLayout, OutOfMemoryError> {
  let create_info = vk::DescriptorSetLayoutCreateInfo {
    s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::DescriptorSetLayoutCreateFlags::empty(),
    binding_count: bindings.len() as u32,
    p_bindings: bindings.as_ptr(),
    _marker: PhantomData,
  };
  unsafe { device.create_descriptor_set_layout(&create_info, None) }.map_err(|err| err.into())
}

fn allocate_sets(
  device: &ash::Device,
  pool: vk::DescriptorPool,
  layouts: &[vk::DescriptorSetLayout],
) -> Result<Vec<vk::DescriptorSet>, OutOfMemoryError> {
  let allocate_info = vk::DescriptorSetAllocateInfo {
    s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
    p_next: ptr::null(),
    descriptor_pool: pool,
    descriptor_set_count: layouts.len() as u32,
    p_set_layouts: layouts.as_ptr(),
    _marker: PhantomData,
  };
  unsafe { device.allocate_descriptor_sets(&allocate_info) }.map_err(|err| {
    match err {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        OutOfMemoryError::from(err)
      }
      vk::Result::ERROR_FRAGMENTED_POOL => {
        panic!("Unexpected fragmentation in pool. Is this application performing reallocations?")
      }
      vk::Result::ERROR_OUT_OF_POOL_MEMORY => {
        // application probably allocated too many sets or SET_COUNT / SIZES is wrong
        panic!("Out of pool memory")
      }
      _ => panic!(),
    }
  })
}
