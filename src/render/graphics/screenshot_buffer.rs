use ash::vk;
use vkallocator::{DetailedMemory, HostMemorySyncError, MappedHostBuffer};
use vkinitialization::device::{Device, PhysicalDevice};
use vkobjects::{destroy, utility::OnErr, DeviceManuallyDestroyed};

use crate::render::{
  create_objs::create_buffer, errors::GPUDataAllocationError, IMAGE_WITH_RESOLUTION_MINIMAL_SIZE,
};

pub struct ScreenshotBuffer {
  pub buffer: MappedHostBuffer<u8>,
  mem: DetailedMemory,
}

impl ScreenshotBuffer {
  const PRIORITY: f32 = 0.2;
  const BUFFER_SIZE: u64 = IMAGE_WITH_RESOLUTION_MINIMAL_SIZE;

  // todo: change error name
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let buffer = create_buffer(
      &device,
      Self::BUFFER_SIZE,
      vk::BufferUsageFlags::TRANSFER_DST,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"screenshot buffer",
    )?;

    let (alloc, host_objects) = vkallocator::allocate_and_map_host_memory(
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
    let buffer = host_objects[0].into_buffer();

    Ok(Self { buffer, mem })
  }

  pub unsafe fn read_memory(&self, device: &ash::Device) -> Result<Box<[u8]>, HostMemorySyncError> {
    self.buffer.invalidate_memory_range(device)?;
    Ok(self.buffer.read_to_box(Self::BUFFER_SIZE as usize))
  }
}

impl DeviceManuallyDestroyed for ScreenshotBuffer {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.buffer.destroy_self(device);
    self.mem.destroy_self(device);
  }
}
