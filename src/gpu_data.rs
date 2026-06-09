use std::{mem::size_of, ops::BitOr};

use ash::vk;
use vkallocator::{
  self, AllocationError, DetailedMemory, DeviceMemoryInitializationError, HostAllocationError,
  HostMemorySyncError, MappedHostBuffer, SingleUseStagingBuffers,
};
use vkobjects::{
  destroy,
  errors::{OutOfMemoryError, QueueSubmitError},
  utility::OnErr,
  DeviceManuallyDestroyed,
};

use crate::{
  command_pools::{self, initialization::PendingInitialization},
  create_objs::{create_buffer, create_image, create_image_view},
  render_pass::create_framebuffer,
  vertices::Vertex,
  INDICES, VERTICES,
};

use vkinitialization::device::{Device, PhysicalDevice, SingleQueues};

static VERTEX_SIZE: u64 = (size_of::<Vertex>() * VERTICES.len()) as u64;
static INDEX_SIZE: u64 = (size_of::<u16>() * INDICES.len()) as u64;

#[derive(Debug, thiserror::Error)]
pub enum GPUDataAllocationError {
  #[error(transparent)]
  StagingBufferError(#[from] DeviceMemoryInitializationError),
  #[error("Failed to allocate one of the main device memory objects.\n{0}")]
  AllocationError(#[from] AllocationError),
  #[error("Failed to allocate one of the main host memory objects.\n{0}")]
  HostAllocationError(#[from] HostAllocationError),
  #[error(transparent)]
  OutOfMemory(#[from] OutOfMemoryError),
}

#[derive(Debug)]
pub struct GPUData {
  pub render_target: vk::Image,
  pub r_target_image_view: vk::ImageView,
  pub r_target_framebuffer: vk::Framebuffer,

  pub vertex_buffer: vk::Buffer,
  pub index_buffer: vk::Buffer,

  pub host_output_buffer: MappedHostBuffer<u8>,

  memories: Vec<DetailedMemory>,
}

#[must_use]
#[derive(Debug)]
pub struct PendingDataInitialization {
  command_buffer_submit: PendingInitialization,
  staging_buffers: SingleUseStagingBuffers<2>,
}

impl PendingDataInitialization {
  // should not fail
  pub unsafe fn wait_and_self_destroy(&self, device: &ash::Device) -> Result<(), QueueSubmitError> {
    self.command_buffer_submit.wait_and_self_destroy(device)?;
    self.staging_buffers.destroy_self(device);
    Ok(())
  }
}

impl DeviceManuallyDestroyed for PendingDataInitialization {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    log::warn!("Aborting and destroying PendingDataInitialization");
    if let Err(err) = self.wait_and_self_destroy(device) {
      log::error!("PendingDataInitialization failed to destroy self: {}", err);
    }
  }
}

fn create_and_copy_from_staging_buffers(
  device: &Device,
  physical_device: &PhysicalDevice,
  queues: &SingleQueues,
  vertex_buffer: vk::Buffer,
  index_buffer: vk::Buffer,
  #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
) -> Result<PendingDataInitialization, DeviceMemoryInitializationError> {
  let graphics_pool = command_pools::initialization::InitCommandBufferPool::new(
    device,
    physical_device.queue_families.graphics.index,
    #[cfg(feature = "vl")]
    marker,
  )?;

  unsafe {
    let staging_buffers = vkallocator::create_single_use_staging_buffers(
      device,
      physical_device,
      [
        (VERTICES.as_ptr() as *const u8, VERTEX_SIZE),
        (INDICES.as_ptr() as *const u8, INDEX_SIZE),
      ],
      #[cfg(feature = "log_alloc")]
      "DEVICE LOCAL OBJECTS",
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| graphics_pool.destroy_self(device))?;

    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[0],
      vertex_buffer,
      VERTEX_SIZE,
    );
    graphics_pool.record_copy_staging_buffer_to_buffer(
      device,
      staging_buffers.buffers[1],
      index_buffer,
      INDEX_SIZE,
    );

    let submit = graphics_pool
      .end_and_submit(
        device,
        queues.graphics.handle,
        #[cfg(feature = "vl")]
        marker,
      )
      .on_err(|(pool, _err)| destroy!(device => &staging_buffers, pool))
      .map_err(|(_, err)| err)?;

    Ok(PendingDataInitialization {
      command_buffer_submit: submit,
      staging_buffers,
    })
  }
}

impl GPUData {
  pub fn new(
    device: &Device,
    physical_device: &PhysicalDevice,
    render_pass: vk::RenderPass,
    render_extent: vk::Extent2D,
    output_size: u64,
    queues: &SingleQueues,
    #[cfg(feature = "vl")] marker: &vkinitialization::DebugUtilsMarker,
  ) -> Result<(Self, PendingDataInitialization), GPUDataAllocationError> {
    let render_target = create_image(
      device,
      render_extent.width,
      render_extent.height,
      vk::ImageUsageFlags::COLOR_ATTACHMENT.bitor(vk::ImageUsageFlags::TRANSFER_SRC),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"render_target",
    )?;
    let vertex_buffer = create_buffer(
      device,
      VERTEX_SIZE,
      vk::BufferUsageFlags::VERTEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"vertex_buffer",
    )
    .on_err(|_| unsafe { render_target.destroy_self(device) })?;
    let index_buffer = create_buffer(
      device,
      INDEX_SIZE,
      vk::BufferUsageFlags::INDEX_BUFFER.bitor(vk::BufferUsageFlags::TRANSFER_DST),
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"index_buffer",
    )
    .on_err(|_| unsafe { destroy!(device => &vertex_buffer, &render_target) })?;

    let device_alloc = vkallocator::allocate_and_bind_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
        vk::MemoryPropertyFlags::empty(),
      ],
      [&vertex_buffer, &index_buffer, &render_target],
      0.5,
      false,
      #[cfg(feature = "log_alloc")]
      Some(["Vertex buffer", "Index buffer", "Target image"]),
      #[cfg(feature = "log_alloc")]
      "DEVICE LOCAL OBJECTS",
    )
    .on_err(|_| unsafe { destroy!(device => &index_buffer, &vertex_buffer, &render_target) })?;

    let pending_device_init = create_and_copy_from_staging_buffers(
      device,
      physical_device,
      queues,
      vertex_buffer,
      index_buffer,
      #[cfg(feature = "vl")]
      marker,
    )
    .on_err(|_| unsafe {
      destroy!(device => &index_buffer, &vertex_buffer, &render_target, &device_alloc)
    })?;

    let host_output_buffer = create_buffer(device, output_size, vk::BufferUsageFlags::TRANSFER_DST,
      #[cfg(feature = "vl")]
      marker,
      #[cfg(feature = "vl")]
      c"host_output_buffer",
    )
    .on_err(|_| unsafe { destroy!(device => &render_target, &pending_device_init, &index_buffer, &vertex_buffer, &device_alloc) })?;

    let (host_output_buffer_alloc, mapped_objs) = vkallocator::allocate_and_map_host_memory(
      device,
      physical_device,
      [
        vk::MemoryPropertyFlags::HOST_VISIBLE.bitor(vk::MemoryPropertyFlags::HOST_CACHED),
        vk::MemoryPropertyFlags::HOST_VISIBLE,
      ],
      [&host_output_buffer],
      0.5,
      #[cfg(feature = "log_alloc")]
      Some(["Buffer where the final data is read from"]),
      #[cfg(feature = "log_alloc")]
      "OUTPUT BUFFER",
    )
    .on_err(|_| unsafe {destroy!(device => &host_output_buffer, &render_target, &pending_device_init, &index_buffer, &vertex_buffer, &device_alloc) })?;
    let host_output_buffer = mapped_objs[0].into_buffer();

    let mut memories = Vec::with_capacity(
      device_alloc.get_memories().len() + host_output_buffer_alloc.get_memories().len(),
    );
    memories.extend_from_slice(device_alloc.get_memories());
    memories.extend_from_slice(host_output_buffer_alloc.get_memories());
    log::info!("Allocated memory count: {}", memories.len());

    let r_target_image_view = create_image_view(device, render_target)
    .on_err(|_| unsafe {destroy!(device => &host_output_buffer, &render_target, &pending_device_init, &index_buffer, &vertex_buffer, memories.as_slice()) })?;

    let r_target_framebuffer = create_framebuffer(
      device,
      render_pass,
      r_target_image_view,
      render_extent,
    ).on_err(|_| unsafe {
      destroy!(device => &r_target_image_view, &host_output_buffer, &render_target, &pending_device_init, &index_buffer, &vertex_buffer, memories.as_slice()) })?;

    Ok((
      Self {
        render_target,
        r_target_framebuffer,
        r_target_image_view,
        vertex_buffer,
        index_buffer,
        host_output_buffer,
        memories,
      },
      pending_device_init,
    ))
  }

  // returns a slice representing buffer contents after all operations have completed
  pub unsafe fn read_buffer_after_completion(
    &self,
    device: &ash::Device,
    output_size: usize,
  ) -> Result<Box<[u8]>, HostMemorySyncError> {
    self.host_output_buffer.invalidate_memory_range(device)?;

    Ok(self.host_output_buffer.read_to_box(output_size))
  }
}

impl DeviceManuallyDestroyed for GPUData {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    self.r_target_framebuffer.destroy_self(device);
    self.r_target_image_view.destroy_self(device);
    self.render_target.destroy_self(device);

    self.vertex_buffer.destroy_self(device);
    self.index_buffer.destroy_self(device);

    self.host_output_buffer.destroy_self(device);

    self.memories.destroy_self(device);
  }
}
