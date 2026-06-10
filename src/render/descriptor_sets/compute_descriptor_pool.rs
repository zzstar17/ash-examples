use std::{marker::PhantomData, ptr};

use ash::vk;
use vkobjects::{errors::OutOfMemoryError, DeviceManuallyDestroyed};

pub struct ComputeDescriptorPool {
  pub single_storage_buffer_layout: vk::DescriptorSetLayout,
  // 2 sets for a write b and b write a
  // (a -> b, b -> a)
  pub particles_compute: [vk::DescriptorSet; 2],
  pub particles_new: vk::DescriptorSet,
  pub reallocation_sets: [vk::DescriptorSet; 3],

  pool: vk::DescriptorPool,
}

impl ComputeDescriptorPool {
  const SET_COUNT: u32 = 6;

  // each set has 1 storage buffer
  const SIZES: [vk::DescriptorPoolSize; 1] = [vk::DescriptorPoolSize {
    ty: vk::DescriptorType::STORAGE_BUFFER,
    descriptor_count: 6 as u32,
  }];

  const SINGLE_STORAGE_BUFFER_LAYOUT: [vk::DescriptorSetLayoutBinding<'_>; 1] =
    [vk::DescriptorSetLayoutBinding {
      binding: 0,
      descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
      descriptor_count: 1,
      stage_flags: vk::ShaderStageFlags::COMPUTE,
      p_immutable_samplers: ptr::null(),
      _marker: PhantomData,
    }];

  pub fn new(device: &ash::Device) -> Result<Self, OutOfMemoryError> {
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

    let single_storage_buffer_layout = create_layout(device, &Self::SINGLE_STORAGE_BUFFER_LAYOUT)?;

    let layouts = [single_storage_buffer_layout; Self::SET_COUNT as usize];
    let sets = allocate_sets(device, pool, &layouts)?;
    let particles_compute = [sets[0], sets[1]];
    let particles_new = sets[2];
    let reallocation_sets = [sets[3], sets[4], sets[5]];

    Ok(Self {
      single_storage_buffer_layout,
      particles_compute,
      particles_new,
      reallocation_sets,
      pool,
    })
  }

  pub fn update_initial_sets(
    &self,
    device: &ash::Device,
    particles_compute: [vk::Buffer; 2],
    particles_new: vk::Buffer,
  ) {
    // todo: check that range is not bigger than maxStorageBufferRange
    let particles_compute0 = vk::DescriptorBufferInfo {
      buffer: particles_compute[0],
      offset: 0,
      range: vk::WHOLE_SIZE,
    };
    let particles_compute1 = vk::DescriptorBufferInfo {
      buffer: particles_compute[1],
      offset: 0,
      range: vk::WHOLE_SIZE,
    };
    let particles_new = vk::DescriptorBufferInfo {
      buffer: particles_new,
      offset: 0,
      range: vk::WHOLE_SIZE,
    };

    let writes = [
      vk::WriteDescriptorSet {
        dst_set: self.particles_compute[0],
        dst_binding: 0,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
        p_buffer_info: &particles_compute0,
        ..Default::default()
      },
      vk::WriteDescriptorSet {
        dst_set: self.particles_compute[1],
        dst_binding: 0,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
        p_buffer_info: &particles_compute1,
        ..Default::default()
      },
      vk::WriteDescriptorSet {
        dst_set: self.particles_new,
        dst_binding: 0,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::STORAGE_BUFFER,
        p_buffer_info: &particles_new,
        ..Default::default()
      },
    ];

    unsafe {
      device.update_descriptor_sets(&writes, &[]);
    }
  }
}

impl DeviceManuallyDestroyed for ComputeDescriptorPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_descriptor_pool(self.pool, None);
    device.destroy_descriptor_set_layout(self.single_storage_buffer_layout, None);
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
