#[cfg(feature = "vl")]
use std::ffi::CStr;
use std::{marker::PhantomData, ptr};

use ash::vk;
#[cfg(feature = "vl")]
use ash::vk::Handle;
use vkobjects::{destroy, errors::OutOfMemoryError, utility::OnErr, DeviceManuallyDestroyed};

use crate::render::gpu_data::GPUData;

fn create_texture_sampler(
  device: &ash::Device,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
) -> Result<vk::Sampler, OutOfMemoryError> {
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
    border_color: vk::BorderColor::INT_TRANSPARENT_BLACK,
    unnormalized_coordinates: vk::FALSE,
    compare_enable: vk::FALSE,
    compare_op: vk::CompareOp::NEVER,
    mipmap_mode: vk::SamplerMipmapMode::NEAREST,
    mip_lod_bias: 0.0,
    min_lod: 0.0,
    max_lod: vk::LOD_CLAMP_NONE,
    _marker: PhantomData,
  };
  unsafe {
    let sampler = device.create_sampler(&sampler_create_info, None)?;
    #[cfg(feature = "vl")]
    marker.set_obj_name(
      vk::ObjectType::SAMPLER,
      sampler.as_raw(),
      c"Texture Sampler",
    )?;
    Ok(sampler)
  }
}

// lookup nearest float
fn create_sampler_nearest_float(
  device: &ash::Device,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
) -> Result<vk::Sampler, OutOfMemoryError> {
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
    border_color: vk::BorderColor::FLOAT_OPAQUE_BLACK,
    unnormalized_coordinates: vk::TRUE,
    compare_enable: vk::FALSE,
    compare_op: vk::CompareOp::NEVER,
    mipmap_mode: vk::SamplerMipmapMode::NEAREST,
    mip_lod_bias: 0.0,
    max_lod: 0.0,
    min_lod: 0.0,
    _marker: PhantomData,
  };
  unsafe {
    let sampler = device.create_sampler(&sampler_create_info, None)?;
    #[cfg(feature = "vl")]
    marker.set_obj_name(
      vk::ObjectType::SAMPLER,
      sampler.as_raw(),
      c"Nearest float sampler",
    )?;
    Ok(sampler)
  }
}

// lookup nearest int
fn create_sampler_nearest_int(
  device: &ash::Device,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
) -> Result<vk::Sampler, OutOfMemoryError> {
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
  unsafe {
    let sampler = device.create_sampler(&sampler_create_info, None)?;
    #[cfg(feature = "vl")]
    marker.set_obj_name(
      vk::ObjectType::SAMPLER,
      sampler.as_raw(),
      c"Nearest int sampler",
    )?;
    Ok(sampler)
  }
}

pub struct DescriptorPool {
  pub textures_layout: vk::DescriptorSetLayout,
  textures_sampler: vk::Sampler,
  pub textures_sets: Vec<vk::DescriptorSet>,
  pub text_ui_set: vk::DescriptorSet,

  pub text_layout: vk::DescriptorSetLayout,
  pub text_curves_sampler: vk::Sampler,
  pub text_bands_sampler: vk::Sampler,
  pub text_set: vk::DescriptorSet,

  pool: vk::DescriptorPool,
}

impl DescriptorPool {
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

  fn text_layout_bindings<'a>(
    curves_sampler: *const vk::Sampler,
    bands_sampler: *const vk::Sampler,
  ) -> [vk::DescriptorSetLayoutBinding<'a>; 2] {
    [
      vk::DescriptorSetLayoutBinding {
        binding: 0,
        descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: 1,
        stage_flags: vk::ShaderStageFlags::FRAGMENT,
        p_immutable_samplers: curves_sampler,
        _marker: PhantomData,
      },
      vk::DescriptorSetLayoutBinding {
        binding: 1,
        descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: 1,
        stage_flags: vk::ShaderStageFlags::FRAGMENT,
        p_immutable_samplers: bands_sampler,
        _marker: PhantomData,
      },
    ]
  }

  pub fn new(
    device: &ash::Device,
    gpu_data: &GPUData,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, OutOfMemoryError> {
    let textures_sampler = create_texture_sampler(
      device,
      #[cfg(feature = "vl")]
      marker,
    )?;
    let text_curves_sampler = create_sampler_nearest_float(
      device,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_err| unsafe {
      destroy!(device => &textures_sampler);
    })?;
    let text_bands_sampler = create_sampler_nearest_int(
      device,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_err| unsafe {
      destroy!(device => &text_curves_sampler, &textures_sampler);
    })?;

    let textures_layout = Self::create_graphics_layout(
      device,
      textures_sampler,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_err| unsafe {
      destroy!(device => &text_bands_sampler, &text_curves_sampler, &textures_sampler);
    })?;
    let text_layout = Self::create_text_layout(
      device,
      text_curves_sampler,
      text_bands_sampler,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_err| unsafe {
      destroy!(device => &textures_layout, &text_bands_sampler, &text_curves_sampler, &textures_sampler);
    })?;

    // textures, text_ui, text
    let textures_len = gpu_data.sprite_buffers.textures.len() as u32;
    let set_count = textures_len + 2;

    // text has 2
    let sizes: [vk::DescriptorPoolSize; 1] = [vk::DescriptorPoolSize {
      ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      descriptor_count: textures_len + 1 + 2,
    }];

    let pool = {
      let pool_create_info = vk::DescriptorPoolCreateInfo {
        pool_size_count: sizes.len() as u32,
        p_pool_sizes: sizes.as_ptr(),
        max_sets: set_count,
        flags: vk::DescriptorPoolCreateFlags::empty(),
        ..Default::default()
      };
      unsafe { device.create_descriptor_pool(&pool_create_info, None) }
    }.on_err(|_err| unsafe {
      destroy!(device => &text_layout, &textures_layout, &text_bands_sampler, &text_curves_sampler, &textures_sampler);
    })?;

    let mut layouts = Vec::with_capacity(set_count as usize);
    for _i in 0..textures_len {
      layouts.push(textures_layout);
    }
    layouts.push(textures_layout);
    layouts.push(text_layout);

    let sets = allocate_sets(device, pool, &layouts).on_err(|_err| unsafe {
      destroy!(device => &pool, &text_layout, &textures_layout, &text_bands_sampler, &text_curves_sampler, &textures_sampler);
    })?;
    let mut image_infos = Vec::with_capacity(set_count as usize);
    let mut writes = Vec::with_capacity(set_count as usize);
    let mut set_iter = sets.into_iter();

    let mut textures_sets = Vec::with_capacity(textures_len as usize);
    for texture in gpu_data.sprite_buffers.textures.iter() {
      let set = set_iter.next().unwrap();
      image_infos.push(vk::DescriptorImageInfo {
        sampler: vk::Sampler::null(),
        image_view: texture.view,
        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
      });
      writes.push(vk::WriteDescriptorSet {
        dst_set: set,
        dst_binding: 0,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        p_image_info: ptr::null(),
        ..Default::default()
      });
      textures_sets.push(set);
    }

    let text_ui_set = set_iter.next().unwrap();
    image_infos.push(vk::DescriptorImageInfo {
      sampler: vk::Sampler::null(),
      image_view: gpu_data.text_ui.resolved_view,
      image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    });
    writes.push(vk::WriteDescriptorSet {
      dst_set: text_ui_set,
      dst_binding: 0,
      dst_array_element: 0,
      descriptor_count: 1,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      p_image_info: ptr::null(),
      ..Default::default()
    });

    let text_set = set_iter.next().unwrap();
    image_infos.push(vk::DescriptorImageInfo {
      sampler: vk::Sampler::null(),
      image_view: gpu_data.text_curve_view,
      image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    });
    writes.push(vk::WriteDescriptorSet {
      dst_set: text_set,
      dst_binding: 0,
      dst_array_element: 0,
      descriptor_count: 1,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      p_image_info: ptr::null(),
      ..Default::default()
    });
    image_infos.push(vk::DescriptorImageInfo {
      sampler: vk::Sampler::null(),
      image_view: gpu_data.text_band_view,
      image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    });
    writes.push(vk::WriteDescriptorSet {
      dst_set: text_set,
      dst_binding: 1,
      dst_array_element: 0,
      descriptor_count: 1,
      descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
      p_image_info: ptr::null(),
      ..Default::default()
    });

    unsafe {
      let image_infos_ptr = image_infos.as_ptr();
      for i in 0..writes.len() {
        writes[i].p_image_info = image_infos_ptr.add(i);
      }

      device.update_descriptor_sets(&writes, &[]);
    }

    Ok(Self {
      textures_sampler,
      textures_layout,
      textures_sets,
      text_ui_set,

      text_layout,
      text_curves_sampler,
      text_bands_sampler,
      text_set,
      pool,
    })
  }

  fn create_graphics_layout(
    device: &ash::Device,
    texture_sampler: vk::Sampler,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<vk::DescriptorSetLayout, OutOfMemoryError> {
    let ptr = &texture_sampler;
    let bindings = Self::graphics_layout_bindings(ptr);
    create_layout(
      device,
      &bindings,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Graphics Layout",
    )
  }

  fn create_text_layout(
    device: &ash::Device,
    curves_sampler: vk::Sampler,
    bands_sampler: vk::Sampler,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<vk::DescriptorSetLayout, OutOfMemoryError> {
    let curves_sampler_ptr = &curves_sampler;
    let bands_sampler_ptr = &bands_sampler;
    let bindings = Self::text_layout_bindings(curves_sampler_ptr, bands_sampler_ptr);
    create_layout(
      device,
      &bindings,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Text Layout",
    )
  }
}

impl DeviceManuallyDestroyed for DescriptorPool {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    device.destroy_descriptor_pool(self.pool, None);

    device.destroy_descriptor_set_layout(self.textures_layout, None);
    device.destroy_sampler(self.textures_sampler, None);

    device.destroy_descriptor_set_layout(self.text_layout, None);
    device.destroy_sampler(self.text_curves_sampler, None);
    device.destroy_sampler(self.text_bands_sampler, None);
  }
}

fn create_layout(
  device: &ash::Device,
  bindings: &[vk::DescriptorSetLayoutBinding],
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  #[cfg(feature = "vl")] name: &CStr,
) -> Result<vk::DescriptorSetLayout, OutOfMemoryError> {
  let create_info = vk::DescriptorSetLayoutCreateInfo {
    s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
    p_next: ptr::null(),
    flags: vk::DescriptorSetLayoutCreateFlags::empty(),
    binding_count: bindings.len() as u32,
    p_bindings: bindings.as_ptr(),
    _marker: PhantomData,
  };
  unsafe {
    let layout = device.create_descriptor_set_layout(&create_info, None)?;
    #[cfg(feature = "vl")]
    marker.set_obj_name(vk::ObjectType::DESCRIPTOR_SET_LAYOUT, layout.as_raw(), name)?;
    Ok(layout)
  }
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
