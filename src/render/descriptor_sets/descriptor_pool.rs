use std::{marker::PhantomData, ptr};

use ash::vk;

use crate::{
  render::{
    data::compute::ComputeData, device_destroyable::DeviceManuallyDestroyed,
    errors::OutOfMemoryError, FRAMES_IN_FLIGHT,
  },
  utility::{self, concatenate_arrays},
};

use super::{
  storage_buffer_descriptor_set, texture_write_descriptor_set,
  writes::uniform_buffer_descriptor_set,
};

fn create_texture_sampler(device: &ash::Device) -> Result<vk::Sampler, OutOfMemoryError> {
  let sampler_create_info = vk::SamplerCreateInfo {
    s_type: vk::StructureType::SAMPLER_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::SamplerCreateFlags::empty(),
    mag_filter: vk::Filter::NEAREST,
    min_filter: vk::Filter::NEAREST,
    address_mode_u: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    address_mode_v: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    address_mode_w: vk::SamplerAddressMode::CLAMP_TO_BORDER,
    anisotropy_enable: vk::FALSE,
    max_anisotropy: 0.0,
    border_color: vk::BorderColor::INT_OPAQUE_BLACK,
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

  pub compute_layout: vk::DescriptorSetLayout,
  pub compute_sets: [vk::DescriptorSet; FRAMES_IN_FLIGHT],

  pool: vk::DescriptorPool,
}

impl DescriptorPool {
  // graphics layout
  // set count: 1
  // descriptors per set:
  //   - 1 COMBINED_IMAGE_SAMPLER:
  //      - texture

  // compute layout
  // set count: FRAMES_IN_FLIGHT
  // descriptors per set:
  //   - 3 STORAGE_BUFFER:
  //      - output (host visible shader output)
  //      - instance data src (result from previous frame)
  //      - instance data dst (shader result)
  //   - 1 UNIFORM_BUFFER:
  //      - random values ready only

  // this pool only allocates one set and does not reallocate
  const SET_COUNT: u32 = Self::SIZES[0].descriptor_count
    + Self::SIZES[1].descriptor_count
    + Self::SIZES[2].descriptor_count;

  const SIZES: [vk::DescriptorPoolSize; 3] = [
    vk::DescriptorPoolSize {
      ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: 1,
    },
    vk::DescriptorPoolSize {
      ty: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: (3 * FRAMES_IN_FLIGHT) as u32,
    },
    vk::DescriptorPoolSize {
      ty: vk::DescriptorType::UNIFORM_BUFFER,
      descriptor_count: (1 * FRAMES_IN_FLIGHT) as u32,
    },
  ];

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

  const COMPUTE_LAYOUT_BINDINGS: [vk::DescriptorSetLayoutBinding<'_>; 4] = [
    vk::DescriptorSetLayoutBinding {
      binding: 0,
      descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      p_immutable_samplers: ptr::null(),
      _marker: PhantomData,
    },
    vk::DescriptorSetLayoutBinding {
      binding: 1,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      p_immutable_samplers: ptr::null(),
      _marker: PhantomData,
    },
    vk::DescriptorSetLayoutBinding {
      binding: 2,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      p_immutable_samplers: ptr::null(),
      _marker: PhantomData,
    },
    vk::DescriptorSetLayoutBinding {
      binding: 3,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      p_immutable_samplers: ptr::null(),
      _marker: PhantomData,
    },
  ];

  pub fn new(
    device: &ash::Device,
    texture_view: vk::ImageView,
    compute_data: &ComputeData,
  ) -> Result<Self, OutOfMemoryError> {
    let texture_sampler = create_texture_sampler(device)?;

    let texture_layout = Self::create_graphics_layout(device, texture_sampler)?;
    let compute_layout = create_layout(device, &Self::COMPUTE_LAYOUT_BINDINGS)?;

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

    let sets = allocate_sets(
      device,
      pool,
      &concatenate_arrays::<3, vk::DescriptorSetLayout>(&[
        &[texture_layout],
        &[compute_layout; FRAMES_IN_FLIGHT],
      ]),
    )?;
    let texture_set = sets[0];
    let compute_sets = [sets[1], sets[2]];

    // update texture set
    let texture_write = texture_write_descriptor_set(texture_set, texture_view, 0);
    let compute_writes = [
      uniform_buffer_descriptor_set(
        compute_sets[0],
        0,
        vk::DescriptorBufferInfo {
          buffer: compute_data.device.random_values[0],
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
      uniform_buffer_descriptor_set(
        compute_sets[1],
        0,
        vk::DescriptorBufferInfo {
          buffer: compute_data.device.random_values[1],
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
      storage_buffer_descriptor_set(
        compute_sets[0],
        1,
        vk::DescriptorBufferInfo {
          buffer: compute_data.host.compute_host_io[0].buffer,
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
      storage_buffer_descriptor_set(
        compute_sets[1],
        1,
        vk::DescriptorBufferInfo {
          buffer: compute_data.host.compute_host_io[1].buffer,
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
      // src 0
      storage_buffer_descriptor_set(
        compute_sets[0],
        2,
        vk::DescriptorBufferInfo {
          buffer: compute_data.device.instance_compute[1],
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
      // dst 0
      storage_buffer_descriptor_set(
        compute_sets[0],
        3,
        vk::DescriptorBufferInfo {
          buffer: compute_data.device.instance_compute[0],
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
      // src 1
      storage_buffer_descriptor_set(
        compute_sets[1],
        2,
        vk::DescriptorBufferInfo {
          buffer: compute_data.device.instance_compute[0],
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
      // dst 1
      storage_buffer_descriptor_set(
        compute_sets[1],
        3,
        vk::DescriptorBufferInfo {
          buffer: compute_data.device.instance_compute[1],
          offset: 0,
          range: vk::WHOLE_SIZE,
        },
      ),
    ];

    unsafe {
      let texture_contextualized = texture_write.contextualize();
      let mut compute_contextualized = [vk::WriteDescriptorSet::default(); 8];
      for i in 0..8 {
        compute_contextualized[i] = compute_writes[i].contextualize();
      }
      // adding copies as well would be faster but here it doesn't really matter as it will only happen once
      device.update_descriptor_sets(
        &utility::concatenate_arrays::<9, vk::WriteDescriptorSet>(&[
          &[texture_contextualized],
          &compute_contextualized,
        ]),
        &[],
      );
    }
    // todo: update compute sets in the same command as well

    Ok(Self {
      texture_sampler,
      texture_layout,
      compute_layout,
      texture_set,
      compute_sets,
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
    self.pool.destroy_self(device);

    self.texture_layout.destroy_self(device);
    self.texture_sampler.destroy_self(device);

    self.compute_layout.destroy_self(device);
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
