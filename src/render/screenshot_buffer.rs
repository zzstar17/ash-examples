use std::{
  marker::PhantomData,
  ptr::{self, NonNull},
};

use ash::vk;
use vkallocator::{DeviceMemoryInitializationError, MemoryWithType};
use vkinitialization::device::{Device, PhysicalDevice};
use vkobjects::{destroy, errors::OutOfMemoryError, utility::OnErr, DeviceManuallyDestroyed};

use crate::render::{create_objs::create_buffer, IMAGE_WITH_RESOLUTION_MINIMAL_SIZE};

pub struct ScreenshotBuffer {
  pub buffer: vk::Buffer,
  ptr: NonNull<u8>,
  mem: MemoryWithType,
}

impl ScreenshotBuffer {
  const PRIORITY: f32 = 0.2;
  const BUFFER_SIZE: u64 = IMAGE_WITH_RESOLUTION_MINIMAL_SIZE;

  // todo: change error name
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, DeviceMemoryInitializationError> {
    let buffer = create_buffer(
      &device,
      Self::BUFFER_SIZE,
      vk::BufferUsageFlags::TRANSFER_DST,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"screenshot buffer",
    )?;

    let alloc = vkallocator::allocate_and_bind_memory(
      device,
      physical_device,
      [vk::MemoryPropertyFlags::HOST_VISIBLE],
      [&buffer],
      Self::PRIORITY,
      #[cfg(feature = "log_alloc")]
      Some(["Screenshot buffer"]),
      #[cfg(feature = "log_alloc")]
      "SCREENSHOT BUFFER",
    )
    .on_err(|_| unsafe { destroy!(device => &buffer) })?;
    let mem = alloc.memories[0];
    let offset = alloc.obj_to_memory_assignment[0].1;

    let ptr = unsafe {
      device
        .map_memory(
          *mem,
          0,
          // if size is not vk::WHOLE_SIZE, mapping should follow alignments
          vk::WHOLE_SIZE,
          vk::MemoryMapFlags::empty(),
        )?
        .byte_add(offset as usize)
    } as *mut u8;
    let ptr = NonNull::new(ptr).unwrap();

    Ok(Self { buffer, ptr, mem })
  }

  pub unsafe fn invalidate_memory(
    &self,
    device: &Device,
    physical_device: &PhysicalDevice,
  ) -> Result<(), OutOfMemoryError> {
    if !physical_device.mem_properties.memory_types[self.mem.type_index]
      .property_flags
      .contains(vk::MemoryPropertyFlags::HOST_COHERENT)
    {
      let range = vk::MappedMemoryRange {
        s_type: vk::StructureType::MAPPED_MEMORY_RANGE,
        p_next: ptr::null(),
        memory: *self.mem,
        offset: 0,
        size: vk::WHOLE_SIZE,
        _marker: PhantomData,
      };
      device.invalidate_mapped_memory_ranges(&[range])?;
    }
    Ok(())
  }

  pub unsafe fn read_memory(&self) -> &[u8] {
    std::slice::from_raw_parts(self.ptr.as_ptr(), Self::BUFFER_SIZE as usize)
  }
}

impl DeviceManuallyDestroyed for ScreenshotBuffer {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.buffer.destroy_self(device);
    self.mem.destroy_self(device);
  }
}
