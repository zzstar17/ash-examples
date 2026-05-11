use std::{ffi::c_void, marker::PhantomData, ops::Deref, ptr};

use ash::vk;
use mem_type_assignment::{
  assign_memory_type_indexes_to_objects_for_allocation, MemoryAssignmentError,
  UnassignedToMemoryObjectsData,
};
use vkobjects::{
  errors::OutOfMemoryError,
  utility::{self, OnErr},
  DeviceManuallyDestroyed,
};

use vkinitialization::device::{Device, PhysicalDevice};

#[cfg(feature = "log_alloc")]
mod logging;
mod mem_type_assignment;
mod memory_bound;
mod staging_buffers;

pub use memory_bound::MemoryBound;
pub use staging_buffers::{
  create_single_use_staging_buffers, DeviceMemoryInitializationError, SingleUseStagingBuffers,
};

#[allow(unused_imports)]
#[cfg(feature = "log_alloc")]
pub use logging::{debug_print_device_memory_info, debug_print_possible_memory_type_assignment};

#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryWithType {
  pub memory: vk::DeviceMemory,
  pub type_index: usize,
}

impl Deref for MemoryWithType {
  type Target = vk::DeviceMemory;

  fn deref(&self) -> &Self::Target {
    &self.memory
  }
}

impl DeviceManuallyDestroyed for MemoryWithType {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.memory.destroy_self(device);
  }
}

#[derive(Debug, Clone, Copy)]
pub struct AllocationSuccess<const S: usize> {
  pub memories: [MemoryWithType; vk::MAX_MEMORY_TYPES],
  pub memory_count: usize,
  // memory index, offset
  pub obj_to_memory_assignment: [(usize, u64); S],
}

impl<const S: usize> AllocationSuccess<S> {
  pub fn get_memories(&self) -> &[MemoryWithType] {
    &self.memories[0..self.memory_count]
  }
}

impl<const S: usize> DeviceManuallyDestroyed for AllocationSuccess<S> {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.get_memories().destroy_self(device);
  }
}

#[derive(Debug, thiserror::Error, Clone, Copy)]
pub enum AllocationError {
  #[error("Object to memory type assignment did not succeed")]
  AssignmentError(#[from] MemoryAssignmentError),
  #[error(transparent)]
  OutOfMemoryError(#[from] OutOfMemoryError),
}

// todo: not checking heap capacity, maxMemoryAllocationSize
// (https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VK_AMD_memory_overallocation_behavior.html)
pub fn allocate_memory<const P: usize, const S: usize>(
  device: &Device,
  physical_device: &PhysicalDevice,
  mem_props: [vk::MemoryPropertyFlags; P],
  objs: [&dyn MemoryBound; S],
  priority: f32, // only set if VK_EXT_memory_priority is enabled
  #[cfg(feature = "log_alloc")] obj_labels: Option<[&'static str; S]>,
  #[cfg(feature = "log_alloc")] allocation_name: &str,
) -> Result<AllocationSuccess<S>, AllocationError> {
  let mem_types = physical_device.memory_types();
  let obj_reqs = unsafe { objs.map(|obj| obj.get_memory_requirements(device)) };

  let assign_result =
    assign_memory_type_indexes_to_objects_for_allocation(UnassignedToMemoryObjectsData {
      mem_types,
      mem_props,
      obj_reqs,
      #[cfg(feature = "log_alloc")]
      obj_labels,
    });

  #[cfg(feature = "log_alloc")]
  {
    let labels: Option<&[&str]> = match &obj_labels {
      Some(labels_arr) => Some(labels_arr.as_slice()),
      None => None,
    };
    let mut output = String::new();
    logging::display_mem_assignment_result::<P, S>(
      &mut output,
      assign_result,
      allocation_name,
      mem_types,
      &mem_props,
      &obj_reqs,
      labels,
    )
    .unwrap();
    let _ = output.pop(); // remove last \n
    if assign_result.is_ok() {
      log::debug!("{}", output);
    } else {
      log::error!("{}", output);
    }
  }

  let (assigned, unique_type_ixs_count) = assign_result?;

  let mut working_memory_types = [usize::MAX; vk::MAX_MEMORY_TYPES];
  let mut working_memory_types_size = 0;
  for type_i in assigned {
    if !working_memory_types[0..working_memory_types_size].contains(&type_i) {
      working_memory_types[working_memory_types_size] = type_i;
      working_memory_types_size += 1;
    }
  }
  debug_assert_eq!(working_memory_types_size, unique_type_ixs_count);

  // mem index, offset
  let mut allocation_result = [(usize::MAX, u64::MAX); S];
  let mut memories = [MemoryWithType {
    memory: vk::DeviceMemory::null(),
    type_index: usize::MAX,
  }; vk::MAX_MEMORY_TYPES];
  for (mem_i, &type_i) in working_memory_types[0..working_memory_types_size]
    .iter()
    .enumerate()
  {
    let mut total_size = 0;
    for i in 0..S {
      if assigned[i] == type_i {
        let offset: u64 = utility::round_up_to_power_of_2_u64(total_size, obj_reqs[i].alignment);
        total_size = offset + obj_reqs[i].size;
        allocation_result[i].1 = offset;
      }
    }

    // todo: this probably would require dividing the allocations
    assert!(total_size <= physical_device.properties.max_memory_allocation_size);

    let mut allocate_info = vk::MemoryAllocateInfo {
      s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
      p_next: ptr::null(),
      allocation_size: total_size,
      memory_type_index: type_i as u32,
      _marker: PhantomData,
    };
    let priority_info = vk::MemoryPriorityAllocateInfoEXT::default().priority(priority);
    if device.enabled_extensions.memory_priority {
      allocate_info.p_next =
        &priority_info as *const vk::MemoryPriorityAllocateInfoEXT as *const c_void;
    }
    let memory = unsafe { device.allocate_memory(&allocate_info, None) }
      .on_err(|_| unsafe {
        for &mem in memories[0..mem_i].iter() {
          mem.destroy_self(device);
        }
      })
      .map_err(|vkerr| AllocationError::OutOfMemoryError(OutOfMemoryError::from(vkerr)))?;
    if let Some(loader) = device.pageable_device_local_memory_loader.as_ref() {
      unsafe {
        // set manually priority as well for the pageable_device_local_memory extension
        // only raw function pointers for now
        (loader.fp().set_device_memory_priority_ext)(device.handle(), memory, priority);
      }
    }
    memories[mem_i] = MemoryWithType {
      memory,
      type_index: type_i,
    };

    for i in 0..S {
      if assigned[i] == type_i {
        allocation_result[i].0 = mem_i;
      }
    }
  }

  Ok(AllocationSuccess {
    memories,
    memory_count: working_memory_types_size,
    obj_to_memory_assignment: allocation_result,
  })
}

pub fn allocate_and_bind_memory<const P: usize, const S: usize>(
  device: &Device,
  physical_device: &PhysicalDevice,
  mem_props: [vk::MemoryPropertyFlags; P],
  objs: [&dyn MemoryBound; S],
  priority: f32, // only set if VK_EXT_memory_priority is enabled
  #[cfg(feature = "log_alloc")] obj_labels: Option<[&'static str; S]>,
  #[cfg(feature = "log_alloc")] allocation_name: &str,
) -> Result<AllocationSuccess<S>, AllocationError> {
  let alloc = allocate_memory(
    device,
    physical_device,
    mem_props,
    objs,
    priority,
    #[cfg(feature = "log_alloc")]
    obj_labels,
    #[cfg(feature = "log_alloc")]
    allocation_name,
  )?;
  let memories = alloc.get_memories();

  for (i, &(mem_index, offset)) in alloc.obj_to_memory_assignment.iter().enumerate() {
    unsafe {
      objs[i]
        .bind(device, *memories[mem_index], offset)
        .on_err(|_| alloc.destroy_self(device))?;
    }
  }

  Ok(alloc)
}
