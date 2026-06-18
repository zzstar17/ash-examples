use std::{
  ops::BitOr,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
};

use ash::vk;
use vkobjects::{errors::OutOfMemoryError, utility::OnErr, DeviceManuallyDestroyed};

use crate::{compute::ComputeGPUData, render::create_objs::create_buffer};

// at most 3 buffers owned by graphics and 1 by compute
const BUFFER_COUNT: usize = 4;

#[derive(Clone)]
pub struct ParticleBuffers {
  pub in_use_by_graphics: Arc<[AtomicBool; BUFFER_COUNT]>,
  pub buffers: [vk::Buffer; BUFFER_COUNT],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticleUse {
  Uninitialized,
  Graphics,
  Compute,
  ComputeFinished,
}

#[derive(Debug)]
pub struct ParticleManager {
  in_use_by_graphics: Arc<[AtomicBool; BUFFER_COUNT]>,
  local: [ParticleUse; BUFFER_COUNT],
  last_i: usize,
  compute_i_in_use: Option<usize>,
}

impl ParticleManager {
  pub fn new(in_use_by_graphics: Arc<[AtomicBool; BUFFER_COUNT]>) -> Self {
    Self {
      in_use_by_graphics,
      local: [ParticleUse::Uninitialized; BUFFER_COUNT],
      last_i: BUFFER_COUNT - 1,
      compute_i_in_use: None,
    }
  }

  #[inline]
  pub fn next_i(&mut self) {
    self.last_i += 1;
    if self.last_i == BUFFER_COUNT {
      self.last_i = 0;
    }
  }

  pub fn get_next_compute_i(&mut self) -> usize {
    for _ in 0..BUFFER_COUNT {
      self.next_i();

      // only one
      if self.local[self.last_i] == ParticleUse::Compute {
        panic!("Multiple particle buffers in use by compute");
      }

      // if still in use by graphics then continue
      if self.local[self.last_i] == ParticleUse::Graphics
        && self.in_use_by_graphics[self.last_i].load(Ordering::Acquire)
      {
        continue;
      }

      self.local[self.last_i] = ParticleUse::Compute;
      self.compute_i_in_use = Some(self.last_i);
      return self.last_i;
    }
    panic!("All particle buffers are currently owned by graphics");
  }

  pub fn compute_finished(&mut self) {
    if let Some(compute_i) = self.compute_i_in_use {
      self.local[compute_i] = ParticleUse::ComputeFinished;
    }
  }

  pub fn compute_fail(&mut self) {
    if let Some(compute_i) = self.compute_i_in_use {
      self.local[compute_i] = ParticleUse::Uninitialized;
    }
    self.compute_i_in_use = None;
  }

  // finds first ready buffer and then resets i
  pub fn get_and_mark_next_graphics(&mut self) -> Option<usize> {
    let start_i = self.last_i;
    for _ in 0..BUFFER_COUNT {
      self.next_i();

      if self.local[self.last_i] == ParticleUse::ComputeFinished {
        let cur_i = self.last_i;
        self.in_use_by_graphics[cur_i].store(true, Ordering::Release);
        self.local[cur_i] = ParticleUse::Graphics;
        self.last_i = start_i;
        return Some(cur_i);
      }
    }
    None
  }

  pub fn unmark_graphics(&mut self, i: usize) {
    debug_assert_eq!(self.local[i], ParticleUse::Graphics);
    self.in_use_by_graphics[i].store(false, Ordering::Relaxed);
    // only set to graphics if it was previously ComputeFinished
    self.local[i] = ParticleUse::ComputeFinished;
  }
}

impl ParticleBuffers {
  pub const BUFFER_COUNT: usize = BUFFER_COUNT;

  pub fn new(
    device: &ash::Device,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<Self, OutOfMemoryError> {
    let in_use_by_graphics = Arc::new([
      AtomicBool::new(false),
      AtomicBool::new(false),
      AtomicBool::new(false),
      AtomicBool::new(false),
    ]);

    let buffer_0 = create_buffer(
      device,
      ComputeGPUData::INITIAL_SIZE,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles graphics 0",
    )?;
    let buffer_1 = create_buffer(
      device,
      ComputeGPUData::INITIAL_SIZE,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles graphics 1",
    )
    .on_err(|_| unsafe {
      buffer_0.destroy_self(device);
    })?;
    let buffer_2 = create_buffer(
      device,
      ComputeGPUData::INITIAL_SIZE,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles graphics 2",
    )
    .on_err(|_| unsafe {
      buffer_0.destroy_self(device);
      buffer_1.destroy_self(device);
    })?;
    let buffer_3 = create_buffer(
      device,
      ComputeGPUData::INITIAL_SIZE,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"Particles graphics 2",
    )
    .on_err(|_| unsafe {
      buffer_0.destroy_self(device);
      buffer_1.destroy_self(device);
      buffer_2.destroy_self(device);
    })?;
    let buffers = [buffer_0, buffer_1, buffer_2, buffer_3];

    Ok(Self {
      in_use_by_graphics,
      buffers,
    })
  }
}

impl DeviceManuallyDestroyed for ParticleBuffers {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.buffers.destroy_self(device);
  }
}
