use std::{ops::BitOr, ptr::NonNull};

use ash::vk;
use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};
use vkobjects::{utility::OnErr, DeviceManuallyDestroyed};

use vkallocator::{DetailedMemory, DeviceMemoryInitializationError, MappedHostBuffer};

use crate::render::create_objs::create_buffer;

const INITIAL_CAPACITY: u64 = 100;
const INITIAL_SIZE: u64 = INITIAL_CAPACITY * size_of::<Particle>() as u64;

// size and alignment: 4
#[repr(C)]
#[derive(Debug, Copy, Clone, Default)]
pub struct Particle {
  pos: [f32; 2],
  vel: [f32; 2],
}

#[derive(Debug)]
pub struct GPUData {
  // fully owned by compute
  pub particles_compute: [vk::Buffer; 2],
  // copied to graphics
  pub particles_graphics: [vk::Buffer; 2],
  // read from cpu
  pub particles_from_cpu_read: MappedHostBuffer<Particle>,
  // write to cpu
  pub from_cpu_write: MappedHostBuffer<u8>,

  memories: Vec<DetailedMemory>,
}

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    _queues: &SingleQueues,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, DeviceMemoryInitializationError> {
    let particles_compute_0 = create_buffer(
      device,
      INITIAL_SIZE,
      vk::BufferUsageFlags::STORAGE_BUFFER
        .bitor(vk::BufferUsageFlags::TRANSFER_SRC)
        .bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles compute 0",
    )?;
    let particles_compute_1 = create_buffer(
      device,
      INITIAL_SIZE,
      vk::BufferUsageFlags::STORAGE_BUFFER
        .bitor(vk::BufferUsageFlags::TRANSFER_SRC)
        .bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles compute 1",
    )
    .on_err(|_| unsafe { particles_compute_0.destroy_self(device) })?;
    let particles_compute = [particles_compute_0, particles_compute_1];

    let particles_graphics_0 = create_buffer(
      device,
      INITIAL_SIZE,
      vk::BufferUsageFlags::STORAGE_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles graphics 0",
    )
    .on_err(|_| unsafe { particles_compute.destroy_self(device) })?;
    let particles_graphics_1 = create_buffer(
      device,
      INITIAL_SIZE,
      vk::BufferUsageFlags::STORAGE_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles graphics 1",
    )
    .on_err(|_| unsafe {
      particles_compute.destroy_self(device);
      particles_graphics_0.destroy_self(device)
    })?;
    let particles_graphics = [particles_graphics_0, particles_graphics_1];

    let particles_cpu_read = create_buffer(
      device,
      INITIAL_SIZE,
      vk::BufferUsageFlags::TRANSFER_SRC,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles CPU read",
    )
    .on_err(|_| unsafe {
      particles_compute.destroy_self(device);
      particles_graphics.destroy_self(device)
    })?;
    let cpu_write = create_buffer(
      device,
      INITIAL_SIZE,
      vk::BufferUsageFlags::TRANSFER_DST,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"CPU write",
    )
    .on_err(|_| unsafe {
      particles_compute.destroy_self(device);
      particles_graphics.destroy_self(device);
      particles_cpu_read.destroy_self(device);
    })?;

    let buffers = [
      particles_compute_0,
      particles_compute_1,
      particles_graphics_0,
      particles_graphics_1,
      particles_cpu_read,
      cpu_write,
    ];

    let device_alloc = vkallocator::allocate_and_bind_memory(
      device,
      physical_device,
      [vk::MemoryPropertyFlags::DEVICE_LOCAL],
      [
        &particles_compute_0,
        &particles_compute_1,
        &particles_graphics_0,
        &particles_graphics_1,
      ],
      0.7,
      #[cfg(feature = "log_alloc")]
      Some([
        "Particles compute 0",
        "Particles compute 1",
        "Particles graphics 0",
        "Particles graphics 1",
      ]),
      #[cfg(feature = "log_alloc")]
      "Compute data",
    )
    .on_err(|_| unsafe {
      buffers.destroy_self(device);
    })?;

    let host_map_alloc = vkallocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
        vk::MemoryPropertyFlags::HOST_VISIBLE,
      ],
      [&particles_cpu_read, &cpu_write],
      0.5,
      #[cfg(feature = "log_alloc")]
      Some(["Particles CPU read", "CPU write"]),
      #[cfg(feature = "log_alloc")]
      "Compute data host mapped",
    )
    .on_err(|_| unsafe {
      buffers.destroy_self(device);
      device_alloc.destroy_self(device);
    })?;

    let mut memories =
      Vec::with_capacity(device_alloc.get_memories().len() + host_map_alloc.get_memories().len());
    memories.extend_from_slice(device_alloc.get_memories());
    memories.extend_from_slice(host_map_alloc.get_memories());

    let ptrs: [NonNull<u8>; 2] = vkallocator::map_host_visible_allocation(device, host_map_alloc)
      .on_err(|_| unsafe {
      buffers.destroy_self(device);
      device_alloc.destroy_self(device);
      host_map_alloc.destroy_self(device);
    })?;

    println!("{:?}", ptrs);

    let particles_cpu_read_mapped = MappedHostBuffer {
      buffer: particles_cpu_read,
      data_ptr: NonNull::new(ptrs[0].as_ptr() as *mut Particle).unwrap(),
    };
    let cpu_write_mapped = MappedHostBuffer {
      buffer: cpu_write,
      data_ptr: ptrs[1],
    };

    Ok(Self {
      particles_compute,
      particles_graphics,
      particles_from_cpu_read: particles_cpu_read_mapped,
      from_cpu_write: cpu_write_mapped,
      memories,
    })
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.particles_compute.destroy_self(device);
    self.particles_graphics.destroy_self(device);

    self.particles_from_cpu_read.destroy_self(device);
    self.from_cpu_write.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
