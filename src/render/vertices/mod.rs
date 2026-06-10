mod particle;
mod vertex;

use std::{marker::PhantomData, pin::Pin, ptr};

use ash::vk;

pub use vertex::Vertex;

pub struct PipelineVertexInputStateCreateInfo<'a> {
  _binding_descriptions: Pin<Box<[vk::VertexInputBindingDescription]>>,
  _attribute_descriptions: Pin<Box<[vk::VertexInputAttributeDescription]>>,
  creation_info: vk::PipelineVertexInputStateCreateInfo<'a>,
}

impl<'a> PipelineVertexInputStateCreateInfo<'a> {
  pub fn get(&self) -> &vk::PipelineVertexInputStateCreateInfo<'_> {
    &self.creation_info
  }

  pub fn new(
    bindings: Box<[vk::VertexInputBindingDescription]>,
    attributes: Box<[vk::VertexInputAttributeDescription]>,
  ) -> Self {
    let bindings = Box::into_pin(bindings);
    let attributes = Box::into_pin(attributes);
    let creation_info = vk::PipelineVertexInputStateCreateInfo {
      s_type: vk::StructureType::PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
      p_next: ptr::null(),
      flags: vk::PipelineVertexInputStateCreateFlags::empty(),
      vertex_attribute_description_count: attributes.len() as u32,
      p_vertex_attribute_descriptions: attributes.as_ptr(),
      vertex_binding_description_count: bindings.len() as u32,
      p_vertex_binding_descriptions: bindings.as_ptr(),
      _marker: PhantomData,
    };
    Self {
      _binding_descriptions: bindings,
      _attribute_descriptions: attributes,
      creation_info,
    }
  }
}

// creates a vector with bindings from provided vertex types enumerated using some recursion
// enumerate_arr!(ty1, ty2, ty3);
// gets transformed into
// vec![ty1::get_binding_description(0), ty2::get_binding_description(1), ty3::get_binding_description(2)]
#[macro_export]
macro_rules! enumerate_binding_descriptions {
  // final step
  (@out $($out:expr,)* @step $_i:expr,) => {
    vec![$($out,)*]
  };
  // intermediate step
  (@out $($out:expr,)* @step $i:expr, $head:ty, $($tail:ty,)*) => {
    enumerate_binding_descriptions!(@out $($out,)* <$head>::get_binding_description($i), @step $i + 1u32, $($tail,)*)
  };
  // initial step
  ($($vertices:ty),+) => {
    enumerate_binding_descriptions!(@out @step 0u32, $($vertices,)+)
  }
}
pub use enumerate_binding_descriptions;

// get_attribute_descriptions get called for each vertex type and flattened into a vec
#[macro_export]
macro_rules! enumerate_attribute_descriptions {
  // final step
  (@out $($out:expr,)* @step $_i:expr, $offset:expr, @prev) => {
    {
      let mut result = Vec::with_capacity($offset);
      $(
        result.extend_from_slice(&$out);
      )*
      result
    }
  };
  // intermediate step
  (@out $($out:expr,)* @step $i:expr, $offset:expr, @prev $head:ty, $($tail:ty,)*) => {
    {
      let descriptions = <$head>::get_attribute_descriptions($offset as u32, $i);
      enumerate_attribute_descriptions!(
        @out $($out,)* descriptions, @step $i + 1u32, $offset + descriptions.len(), @prev $($tail,)*
      )
    }
  };
  // initial step
  ($($vertices:ty),+) => {
    enumerate_attribute_descriptions!(@out @step 0u32, 0usize, @prev $($vertices,)+)
  }
}
pub use enumerate_attribute_descriptions;

#[macro_export]
macro_rules! vertex_input_state_create_info {
  ($($vertices:ty),+) => {
    {
    use $crate::render::vertices::{enumerate_binding_descriptions, enumerate_attribute_descriptions, PipelineVertexInputStateCreateInfo};

    let bindings = enumerate_binding_descriptions!($($vertices),+);
    let attributes = enumerate_attribute_descriptions!($($vertices),+);
    PipelineVertexInputStateCreateInfo::new(bindings.into_boxed_slice(), attributes.into_boxed_slice())
    }
  };
}
