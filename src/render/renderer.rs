use std::mem::MaybeUninit;

use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::{
  dpi::PhysicalSize,
  event_loop::EventLoopWindowTarget,
  window::{Window, WindowBuilder},
};

use crate::{
  render::data::constant::create_and_populate_constant_data, utility::OnErr, INITIAL_WINDOW_HEIGHT,
  INITIAL_WINDOW_WIDTH, RESOLUTION, SCREENSHOT_SAVE_FILE, WINDOW_TITLE,
};

use super::{
  command_pools::{
    ComputeCommandPool, GraphicsCommandBufferPool, TemporaryGraphicsCommandPool,
    TransferCommandBufferPool,
  },
  data::{
    compute::{ComputeData, ComputeDataUpdate},
    constant::ConstantData,
    ScreenshotBuffer,
  },
  descriptor_sets::DescriptorPool,
  device_destroyable::{
    destroy, fill_destroyable_array_with_expression, DeviceManuallyDestroyed, ManuallyDestroyed,
  },
  errors::{error_chain_fmt, AllocationError, ImageError, InitializationError, OutOfMemoryError},
  initialization::{
    self,
    device::{Device, PhysicalDevice, Queues},
    Surface,
  },
  pipelines::{self, ComputePipelines, GraphicsPipelines, PipelineCreationError},
  render_pass::create_render_pass,
  render_targets::RenderTargets,
  swapchain::{SwapchainCreationError, Swapchains},
  RenderInit, SpritePushConstants, FRAMES_IN_FLIGHT, RENDER_EXTENT, SWAPCHAIN_IMAGE_USAGES,
};

#[allow(clippy::enum_variant_names)]
#[derive(thiserror::Error)]
pub enum SwapchainRecreationError {
  #[error("Memory error")]
  MemoryError(#[source] AllocationError),
  #[error("Failed to create a swapchain")]
  SwapchainError(#[source] SwapchainCreationError),
  #[error("Failed to create a pipeline")]
  PipelineCreationError(#[source] PipelineCreationError),
}
impl std::fmt::Debug for SwapchainRecreationError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

impl From<OutOfMemoryError> for SwapchainRecreationError {
  fn from(value: OutOfMemoryError) -> Self {
    SwapchainRecreationError::MemoryError(AllocationError::NotEnoughMemory(value))
  }
}

impl From<AllocationError> for SwapchainRecreationError {
  fn from(value: AllocationError) -> Self {
    SwapchainRecreationError::MemoryError(value)
  }
}

impl From<SwapchainCreationError> for SwapchainRecreationError {
  fn from(value: SwapchainCreationError) -> Self {
    SwapchainRecreationError::SwapchainError(value)
  }
}

impl From<PipelineCreationError> for SwapchainRecreationError {
  fn from(value: PipelineCreationError) -> Self {
    SwapchainRecreationError::PipelineCreationError(value)
  }
}

pub struct Renderer {
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: initialization::DebugUtils,
  physical_device: PhysicalDevice,
  pub device: Device,
  pub queues: Queues,

  pub window: Window,
  surface: Surface,

  pub swapchains: Swapchains,

  render_pass: vk::RenderPass,
  render_targets: RenderTargets,

  pipeline_cache: vk::PipelineCache,
  graphics_pipelines: GraphicsPipelines,
  compute_pipelines: ComputePipelines,

  pub graphics_command_pools: [GraphicsCommandBufferPool; FRAMES_IN_FLIGHT],
  pub compute_command_pools: [ComputeCommandPool; FRAMES_IN_FLIGHT],

  constant_data: ConstantData,
  compute_data: ComputeData,
  descriptor_pool: DescriptorPool,

  screenshot_buffer: ScreenshotBuffer,
}

struct Destructor<const N: usize> {
  objs: [MaybeUninit<*const dyn DeviceManuallyDestroyed>; N],
  len: usize,
}

impl<const N: usize> Destructor<N> {
  pub fn new() -> Self {
    Self {
      objs: unsafe { MaybeUninit::uninit().assume_init() },
      len: 0,
    }
  }

  pub fn push(&mut self, ptr: *const dyn DeviceManuallyDestroyed) {
    self.len += 1;
    self.objs[self.len] = MaybeUninit::new(ptr);
  }

  pub unsafe fn fire(&self, device: &ash::Device) {
    for i in (0..self.len).rev() {
      self.objs[i]
        .assume_init()
        .as_ref()
        .unwrap()
        .destroy_self(device);
    }
  }
}

impl Renderer {
  pub fn initialize(
    pre_window: RenderInit,
    target: &EventLoopWindowTarget<()>,
  ) -> Result<Self, InitializationError> {
    // having an error during window creation triggers pre_window drop
    let window = WindowBuilder::new()
      .with_title(WINDOW_TITLE)
      .with_inner_size(PhysicalSize {
        width: INITIAL_WINDOW_WIDTH,
        height: INITIAL_WINDOW_HEIGHT,
      })
      // .with_resizable(false)
      .build(target)?;

    let mut destructor: Destructor<15> = Destructor::new();

    #[cfg(feature = "vl")]
    let (entry, instance, debug_utils) = pre_window.deconstruct();
    #[cfg(not(feature = "vl"))]
    let (entry, instance) = pre_window.deconstruct();

    let destroy_instance = || unsafe {
      #[cfg(feature = "vl")]
      destroy!(&debug_utils);
      destroy!(&instance);
    };
    destructor.push(&instance);
    #[cfg(feature = "vl")]
    destructor.push(&debug_utils);

    let surface = Surface::new(
      &entry,
      &instance,
      target.display_handle()?,
      window.window_handle()?,
    )
    .on_err(|_| destroy_instance())?;
    destructor.push(&surface);

    let physical_device = match unsafe { PhysicalDevice::select(&instance, &surface) }
      .on_err(|_| destroy_instance())?
    {
      Some(device) => device,
      None => {
        destroy_instance();
        return Err(InitializationError::NoCompatibleDevices);
      }
    };

    let (device, queues) =
      Device::create(&instance, &physical_device).on_err(|_| destroy_instance())?;
    destructor.push(&device);

    let swapchains = Swapchains::new(
      &instance,
      &physical_device,
      &device,
      &surface,
      window.inner_size(),
      SWAPCHAIN_IMAGE_USAGES,
    )?;
    destructor.push(&swapchains);

    let render_format = swapchains.get_format(); // same as swapchain
    let render_pass =
      create_render_pass(&device, render_format).on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&render_pass);

    let render_targets = RenderTargets::new(&device, &physical_device, render_pass, render_format)
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    log::debug!("Created render targets:\n{:#?}", render_targets);
    destructor.push(&render_targets);

    log::info!("Creating pipeline cache");
    let (pipeline_cache, created_from_file) =
      pipelines::create_pipeline_cache(&device, &physical_device)
        .on_err(|_| unsafe { destructor.fire(&device) })?;
    if created_from_file {
      log::info!("Cache successfully created from an existing cache file");
    } else {
      log::info!("Cache initialized as empty");
    }
    destructor.push(&pipeline_cache);

    let constant_data = {
      let mut temp_transfer_pool =
        TransferCommandBufferPool::create(&device, &physical_device.queue_families)
          .on_err(|_| unsafe { destructor.fire(&device) })?;
      let mut temp_graphics_pool =
        TemporaryGraphicsCommandPool::create(&device, &physical_device.queue_families).on_err(
          |_| unsafe {
            temp_transfer_pool.destroy_self(&device);
            destructor.fire(&device);
          },
        )?;

      let data = create_and_populate_constant_data(
        &device,
        &physical_device,
        &queues,
        &mut temp_transfer_pool,
        &mut temp_graphics_pool,
      )
      .on_err(|_| unsafe {
        temp_transfer_pool.destroy_self(&device);
        temp_graphics_pool.destroy_self(&device);
        destructor.fire(&device)
      })?;

      log::debug!("GPU <constant data> addresses: {:#?}", data);

      unsafe {
        temp_transfer_pool.destroy_self(&device);
        temp_graphics_pool.destroy_self(&device);
      }

      data
    };

    let compute_data = ComputeData::new(&device, &physical_device)?;

    log::debug!("GPU <compute data> addresses: {:#?}", compute_data);

    let descriptor_pool = DescriptorPool::new(&device, constant_data.texture_view, &compute_data)
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&descriptor_pool);

    log::debug!("Creating pipelines");
    let graphics_pipelines = GraphicsPipelines::new(
      &device,
      pipeline_cache,
      render_pass,
      &descriptor_pool,
      RENDER_EXTENT,
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&graphics_pipelines);

    let compute_pipelines = ComputePipelines::new(&device, pipeline_cache, &descriptor_pool)
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&compute_pipelines);

    log::debug!(
      "Pipeline addresses:\n{:#?}\n{:#?}",
      graphics_pipelines,
      compute_pipelines
    );

    let graphics_command_pools = fill_destroyable_array_with_expression!(
      &device,
      GraphicsCommandBufferPool::create(&device, &physical_device.queue_families),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(graphics_command_pools.as_ptr());

    let compute_command_pools = fill_destroyable_array_with_expression!(
      &device,
      ComputeCommandPool::create(&device, &physical_device.queue_families),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(compute_command_pools.as_ptr());

    let screenshot_buffer = ScreenshotBuffer::new(&device, &physical_device)
      .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&screenshot_buffer);

    Ok(Self {
      window,
      surface,
      _entry: entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      physical_device,
      device,
      queues,
      graphics_command_pools,
      compute_command_pools,
      constant_data,
      compute_data,
      render_pass,
      graphics_pipelines,
      compute_pipelines,
      pipeline_cache,
      swapchains,
      descriptor_pool,
      render_targets,
      screenshot_buffer,
    })
  }

  pub unsafe fn record_graphics(
    &mut self,
    frame_i: usize,
    image_i: usize,
    player: SpritePushConstants,
    effective_instance_size: u64,
    save_to_screenshot_buffer: bool,
  ) -> Result<(), OutOfMemoryError> {
    self.graphics_command_pools[frame_i].reset(&self.device)?;
    self.graphics_command_pools[frame_i].record_main(
      frame_i,
      &self.device,
      &self.physical_device.queue_families,
      self.render_pass,
      &self.render_targets,
      self.swapchains.get_images()[image_i],
      self.swapchains.get_extent(),
      &self.graphics_pipelines,
      &self.descriptor_pool,
      &self.constant_data,
      &self.compute_data,
      player,
      effective_instance_size,
      if save_to_screenshot_buffer {
        Some(*self.screenshot_buffer.buffer)
      } else {
        None
      },
    )?;
    Ok(())
  }

  pub fn update_compute(&mut self, frame_i: usize) -> Result<ComputeDataUpdate, OutOfMemoryError> {
    self
      .compute_data
      .update(frame_i, &self.device, &self.physical_device)
  }

  pub unsafe fn record_compute(
    &mut self,
    frame_i: usize,
    update_data: ComputeDataUpdate,
    player_pos: [f32; 2],
  ) -> Result<u64, OutOfMemoryError> {
    self.compute_command_pools[frame_i].reset(&self.device)?;
    self.compute_command_pools[frame_i]
      .record(
        frame_i,
        &self.device,
        &self.physical_device.queue_families,
        &self.compute_pipelines,
        &self.descriptor_pool,
        &self.compute_data,
        update_data,
        player_pos,
      )
      .map_err(|err| err.into())
  }

  pub unsafe fn recreate_swapchain(&mut self) -> Result<(), SwapchainRecreationError> {
    // most of this function is just cleanup in case of an error

    // it is possible to use more than two frames in flight, but it would require having more than one old swapchain and pipeline
    #[allow(clippy::assertions_on_constants)]
    {
      assert!(FRAMES_IN_FLIGHT == 2);
    }

    // old swapchain becomes retired
    let changes = self.swapchains.recreate(
      &self.physical_device,
      &self.device,
      &self.surface,
      self.window.inner_size(),
      SWAPCHAIN_IMAGE_USAGES,
    )?;

    let mut new_render_pass = None;
    let mut new_render_targets = None;

    if changes.format {
      log::info!("Changing swapchain format");

      // this shouldn't happen regularly, so its okay to stop all rendering so that the render pass can be recreated
      self
        .device
        .device_wait_idle()
        .on_err(|_| self.swapchains.revert_recreate(&self.device))
        .map_err(|vkerr| match vkerr {
          vk::Result::ERROR_OUT_OF_DEVICE_MEMORY | vk::Result::ERROR_OUT_OF_HOST_MEMORY => {
            SwapchainCreationError::OutOfMemory(vkerr.into())
          }
          vk::Result::ERROR_DEVICE_LOST => SwapchainCreationError::DeviceIsLost,
          _ => panic!(),
        })?;

      // recreate all objects that depend on image format (but not on extent)
      let new_format = self.swapchains.get_format();
      new_render_pass = Some(
        create_render_pass(&self.device, new_format)
          .on_err(|_| self.swapchains.revert_recreate(&self.device))?,
      );
      new_render_targets = Some(
        RenderTargets::new(
          &self.device,
          &self.physical_device,
          new_render_pass.unwrap(),
          new_format,
        )
        .on_err(|_| {
          new_render_pass.unwrap().destroy_self(&self.device);
          self.swapchains.revert_recreate(&self.device)
        })?,
      );
    } else if !changes.extent {
      log::warn!("Recreating swapchain without any extent or format change");
    }

    if changes.format {
      match self.graphics_pipelines.recreate(
        &self.device,
        self.pipeline_cache,
        self.render_pass,
        RENDER_EXTENT,
      ) {
        Ok(v) => v,
        Err(err) => unsafe {
          if let Some(render_targets) = new_render_targets {
            render_targets.destroy_self(&self.device);
          }
          if let Some(render_pass) = new_render_pass {
            render_pass.destroy_self(&self.device);
          }
          self.swapchains.revert_recreate(&self.device);

          return Err(err.into());
        },
      }
    }

    if let Some(new) = new_render_pass {
      self.render_pass.destroy_self(&self.device);
      self.render_pass = new;
    }
    if let Some(new) = new_render_targets {
      self.render_targets.destroy_self(&self.device);
      self.render_targets = new;
    }

    Ok(())
  }

  // destroy old objects that resulted of a swapchain recreation
  // this should only be called when they stop being in use
  pub unsafe fn destroy_old(&mut self) {
    self.graphics_pipelines.destroy_old(&self.device);

    self.swapchains.destroy_old(&self.device);
  }

  pub fn render_format(&self) -> vk::Format {
    self.swapchains.get_format()
  }

  // safety: screenshot buffer should not be in use
  pub fn save_screenshot_buffer_as_rgba8(
    &self,
    saved_format: vk::Format,
  ) -> Result<(), ImageError> {
    let ref_slice = unsafe {
      self
        .screenshot_buffer
        .invalidate_memory(&self.device, &self.physical_device)?;
      self.screenshot_buffer.read_memory()
    };

    // copy data to faster memory
    let mut data: Vec<u8> = ref_slice.into();
    let (data_chunks, data_chunks_remainder) = data.as_chunks_mut::<4>();
    assert!(data_chunks_remainder.is_empty());

    // todo: make data save in a separate thread to not stall rendering

    // transform to rgba8
    match saved_format {
      vk::Format::R8G8B8A8_SRGB | vk::Format::R8G8B8A8_UNORM => {}
      vk::Format::B8G8R8A8_SRGB | vk::Format::B8G8R8A8_UNORM => {
        for pixel in data_chunks {
          pixel.swap(0, 2); // swap B and R
        }
      }
      _ => {
        log::error!(
          "Attempting to save screenshot containing an unhandled format: \"{:?}\"",
          saved_format
        );
      }
    }

    image::save_buffer(
      SCREENSHOT_SAVE_FILE,
      &data,
      RESOLUTION[0],
      RESOLUTION[1],
      image::ColorType::Rgba8,
    )?;

    Ok(())
  }
}

impl Drop for Renderer {
  fn drop(&mut self) {
    log::debug!("Destroying renderer objects...");
    unsafe {
      // wait until all operations have finished and the device is safe to destroy
      self
        .device
        .device_wait_idle()
        .expect("Failed to wait for the device to become idle during drop");

      self.destroy_old();

      log::info!("Saving pipeline cache");
      if let Err(err) =
        pipelines::save_pipeline_cache(&self.device, &self.physical_device, self.pipeline_cache)
      {
        log::error!("Failed to save pipeline cache: {:?}", err);
      }

      self.screenshot_buffer.destroy_self(&self.device);

      self.graphics_command_pools.destroy_self(&self.device);
      self.compute_command_pools.destroy_self(&self.device);

      self.graphics_pipelines.destroy_self(&self.device);
      self.compute_pipelines.destroy_self(&self.device);
      self.pipeline_cache.destroy_self(&self.device);
      self.descriptor_pool.destroy_self(&self.device);

      self.constant_data.destroy_self(&self.device);
      self.compute_data.destroy_self(&self.device);

      self.render_targets.destroy_self(&self.device);
      self.render_pass.destroy_self(&self.device);
      self.swapchains.destroy_self(&self.device);

      ManuallyDestroyed::destroy_self(&self.surface);
      ManuallyDestroyed::destroy_self(&self.device);

      #[cfg(feature = "vl")]
      {
        ManuallyDestroyed::destroy_self(&self.debug_utils);
      }
      ManuallyDestroyed::destroy_self(&self.instance);
    }
  }
}
