use ash::vk;
use vkobjects::errors::OutOfMemoryError;

pub trait MemoryBound {
  unsafe fn bind(
    &self,
    device: &ash::Device,
    memory: vk::DeviceMemory,
    offset: u64,
  ) -> Result<(), OutOfMemoryError>;
  unsafe fn get_memory_requirements(&self, device: &ash::Device) -> vk::MemoryRequirements;
}

impl MemoryBound for vk::Buffer {
  unsafe fn bind(
    &self,
    device: &ash::Device,
    memory: vk::DeviceMemory,
    offset: u64,
  ) -> Result<(), OutOfMemoryError> {
    device
      .bind_buffer_memory(*self, memory, offset)
      .map_err(|err| err.into())
  }

  unsafe fn get_memory_requirements(&self, device: &ash::Device) -> vk::MemoryRequirements {
    device.get_buffer_memory_requirements(*self)
  }
}

impl MemoryBound for vk::Image {
  unsafe fn bind(
    &self,
    device: &ash::Device,
    memory: vk::DeviceMemory,
    offset: u64,
  ) -> Result<(), OutOfMemoryError> {
    device
      .bind_image_memory(*self, memory, offset)
      .map_err(|err| err.into())
  }

  unsafe fn get_memory_requirements(&self, device: &ash::Device) -> vk::MemoryRequirements {
    device.get_image_memory_requirements(*self)
  }
}
