use std::ops::BitOr;

use ash::vk;
use rand::RngExt;
use vkinitialization::device::{Device, PhysicalDevice};
use vkobjects::{errors::OutOfMemoryError, utility::OnErr, DeviceManuallyDestroyed};

use vkallocator::{DetailedMemory, HostMemorySyncError, MappedHostBuffer};

use crate::{
  compute::{sync_renderer::COMPUTE_FRAMES_IN_FLIGHT, ParticleBuffers},
  render::{create_objs::create_buffer, vertices::Particle, GPUDataAllocationError},
};

#[derive(Debug)]
pub struct ComputeGPUData {
  // fully owned by compute
  pub particles_compute: [vk::Buffer; COMPUTE_FRAMES_IN_FLIGHT],
  // fully owned by compute
  pub particles_new: vk::Buffer,
  // copied to graphics
  pub particles_graphics: [vk::Buffer; ParticleBuffers::BUFFER_COUNT],
  // read from cpu
  pub from_cpu_read: MappedHostBuffer<Particle>,
  pub particles_from_cpu_read_cur_size: u64,
  // write to cpu
  pub to_cpu_write: MappedHostBuffer<Particle>,

  pub particles_buffer_size: u64,
  pub particles_capacity: u32,
  pub particles_len: u32,
  pub particles_copying: u32,

  memories: Vec<DetailedMemory>,
}

struct Buffers {
  pub particles_compute: [vk::Buffer; COMPUTE_FRAMES_IN_FLIGHT],
  pub particles_new: vk::Buffer,
  pub from_cpu_read: vk::Buffer,
  pub to_cpu_write: vk::Buffer,
}

impl DeviceManuallyDestroyed for Buffers {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.particles_compute.destroy_self(device);
    self.particles_new.destroy_self(device);
    self.from_cpu_read.destroy_self(device);
    self.to_cpu_write.destroy_self(device);
  }
}

impl ComputeGPUData {
  pub const INITIAL_CAPACITY: usize = 64 * 2000;
  pub const INITIAL_SIZE: u64 = (Self::INITIAL_CAPACITY * size_of::<Particle>()) as u64;

  fn create_buffers(
    device: &Device,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Buffers, OutOfMemoryError> {
    let particles_compute_0 = create_buffer(
      device,
      Self::INITIAL_SIZE,
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
      Self::INITIAL_SIZE,
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

    let particles_new = create_buffer(
      device,
      Self::INITIAL_SIZE,
      vk::BufferUsageFlags::STORAGE_BUFFER
        .bitor(vk::BufferUsageFlags::TRANSFER_DST)
        .bitor(vk::BufferUsageFlags::TRANSFER_SRC),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles new",
    )
    .on_err(|_| unsafe { particles_compute.destroy_self(device) })?;

    let particles_cpu_read = create_buffer(
      device,
      Self::INITIAL_SIZE,
      vk::BufferUsageFlags::TRANSFER_SRC,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles CPU read",
    )
    .on_err(|_| unsafe {
      particles_compute.destroy_self(device);
      particles_new.destroy_self(device);
    })?;
    let cpu_write = create_buffer(
      device,
      Self::INITIAL_SIZE,
      vk::BufferUsageFlags::TRANSFER_DST,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"CPU write",
    )
    .on_err(|_| unsafe {
      particles_compute.destroy_self(device);
      particles_new.destroy_self(device);
      particles_cpu_read.destroy_self(device);
    })?;

    Ok(Buffers {
      particles_compute,
      particles_new,
      from_cpu_read: particles_cpu_read,
      to_cpu_write: cpu_write,
    })
  }

  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    // gets owned by this struct
    particles_graphics: [vk::Buffer; ParticleBuffers::BUFFER_COUNT],
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, GPUDataAllocationError> {
    let buffers = Self::create_buffers(
      device,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe {
      particles_graphics.destroy_self(device);
    })?;

    let device_alloc = vkallocator::allocate_and_bind_memory(
      device,
      physical_device,
      [vk::MemoryPropertyFlags::DEVICE_LOCAL],
      [
        &buffers.particles_compute[0],
        &buffers.particles_compute[1],
        &buffers.particles_new,
        &particles_graphics[0],
        &particles_graphics[1],
        &particles_graphics[2],
        &particles_graphics[3],
      ],
      0.7,
      false,
      #[cfg(feature = "log_alloc")]
      Some([
        "Particles compute 0",
        "Particles compute 1",
        "Particles new",
        "Particles graphics 0",
        "Particles graphics 1",
        "Particles graphics 2",
        "Particles graphics 3",
      ]),
      #[cfg(feature = "log_alloc")]
      "Compute data",
    )
    .on_err(|_| unsafe {
      particles_graphics.destroy_self(device);
      buffers.destroy_self(device);
    })?;

    let (host_map_alloc, mapped_host_objs) = vkallocator::allocate_and_map_host_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
        vk::MemoryPropertyFlags::HOST_VISIBLE,
      ],
      [&buffers.from_cpu_read, &buffers.to_cpu_write],
      0.5,
      #[cfg(feature = "log_alloc")]
      Some(["Particles CPU read", "CPU write"]),
      #[cfg(feature = "log_alloc")]
      "Compute data host mapped",
    )
    .on_err(|_| unsafe {
      particles_graphics.destroy_self(device);
      buffers.destroy_self(device);
      device_alloc.destroy_self(device);
    })?;

    let mut memories =
      Vec::with_capacity(device_alloc.get_memories().len() + host_map_alloc.get_memories().len());
    memories.extend_from_slice(device_alloc.get_memories());
    memories.extend_from_slice(host_map_alloc.get_memories());

    let from_cpu_read = mapped_host_objs[0].into_buffer();
    let to_cpu_write = mapped_host_objs[1].into_buffer();

    Ok(Self {
      particles_compute: buffers.particles_compute,
      particles_new: buffers.particles_new,
      particles_graphics,
      from_cpu_read,
      to_cpu_write,
      memories,
      particles_buffer_size: Self::INITIAL_SIZE,
      particles_capacity: Self::INITIAL_CAPACITY as u32,
      particles_len: 0,
      particles_copying: 0,
      particles_from_cpu_read_cur_size: Self::INITIAL_SIZE,
    })
  }

  pub fn current_particles_size(&self) -> u64 {
    self.particles_len as u64 * size_of::<Particle>() as u64
  }

  pub fn current_new_particles_size(&self) -> u64 {
    self.particles_copying as u64 * size_of::<Particle>() as u64
  }

  fn vel_rng(init: f32) -> f32 {
    let mut shifted = (init - 0.5) * 1.6;
    shifted += 0.1 - shifted / 10.0;
    shifted
  }

  pub fn write_particles_to_from_cpu_read(
    &mut self,
    device: &Device,
    new_count: usize,
  ) -> Result<(), HostMemorySyncError> {
    // divide adding new particles somehow?
    assert!(new_count * size_of::<Particle>() <= self.particles_from_cpu_read_cur_size as usize);
    if self.particles_len + new_count as u32 > self.particles_capacity {
      // expand buffer
      todo!();
    }

    let mut particles = Vec::with_capacity(new_count);
    let mut rng: rand::prelude::ThreadRng = rand::rng();
    for _ in 0..new_count {
      particles.push(Particle {
        pos: [
          rng.random::<f32>() - 428.0 * 0.0225,
          rng.random::<f32>() - 283.0 * 0.0225,
        ],
        vel: [
          Self::vel_rng(rng.random::<f32>()),
          Self::vel_rng(rng.random::<f32>()),
        ],
      });
    }

    unsafe {
      self.from_cpu_read.copy_to_buffer_memory(&particles);
      self.from_cpu_read.flush_memory_range(device)?;
    };

    self.particles_copying = new_count as u32;

    Ok(())
  }

  pub fn commit_new_particles(&mut self) {
    self.particles_len += self.particles_copying;
    self.particles_copying = 0;
  }
}

impl DeviceManuallyDestroyed for ComputeGPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.particles_compute.destroy_self(device);
    self.particles_new.destroy_self(device);
    self.particles_graphics.destroy_self(device);

    self.from_cpu_read.destroy_self(device);
    self.to_cpu_write.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
