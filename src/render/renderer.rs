use std::mem::MaybeUninit;

use ash::vk;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use vkallocator::HostMemorySyncError;
use vkinitialization::{
  device::{Device, DeviceExtensions, DeviceFeatures, PhysicalDevice, SingleQueues},
  Surface,
};
use vkobjects::{
  destroy, fill_destroyable_array_with_expression, utility::OnErr, DeviceManuallyDestroyed,
  ManuallyDestroyed,
};
use winit::{dpi::PhysicalSize, event_loop::ActiveEventLoop, window::Window};

use crate::{
  ferris::Ferris,
  font,
  render::{
    command_pools::graphics::GraphicsCommandBufferPool,
    gpu_data::{
      sprite_buffers::SpriteTextureData,
      text::{TextBufferDimensions, TextData},
      GPUDataAllocationError,
    },
    pipelines::TextPipeline,
  },
  slug::{SlugRendering, SlugVertex},
  INITIAL_WINDOW_HEIGHT, INITIAL_WINDOW_WIDTH, RESOLUTION, SCREENSHOT_SAVE_FILE, WINDOW_TITLE,
};

use super::{
  descriptor_sets::DescriptorPool,
  errors::{ImageError, InitializationError, SwapchainRecreationError},
  format_conversions::{self, KNOWN_FORMATS},
  gpu_data::GPUData,
  initialization::{self},
  pipelines::{self, GraphicsPipeline},
  render_object::RenderPosition,
  render_targets::RenderTargets,
  screenshot_buffer::ScreenshotBuffer,
  swapchain::{SwapchainCreationError, Swapchains},
  RenderInit, FRAMES_IN_FLIGHT, RENDER_EXTENT, SWAPCHAIN_IMAGE_USAGES,
};

pub struct Renderer {
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: vkinitialization::DebugUtils,
  #[cfg(feature = "vl")]
  pub debug_utils_marker: vkinitialization::DebugUtilsMarker,
  physical_device: PhysicalDevice,
  pub device: Device,
  pub queues: SingleQueues,

  pub window: Window,
  surface: Surface,

  pub swapchains: Swapchains,
  render_targets: RenderTargets,

  pipeline_cache: vk::PipelineCache,
  pipeline: GraphicsPipeline,
  text_pipeline: TextPipeline,
  pub graphics_pools: [GraphicsCommandBufferPool; FRAMES_IN_FLIGHT],
  // pub device_copy_pool: DeviceCopyCommandBufferPool,
  pub data: GPUData,
  descriptor_pool: DescriptorPool,

  screenshot_buffer: ScreenshotBuffer,

  pub slug: SlugRendering<'static>,
  pub text_vertices: Vec<SlugVertex>,
  pub text_indices: Vec<u32>,
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
    event_loop: &ActiveEventLoop,
    sprite_data: &mut SpriteTextureData,
  ) -> Result<Self, InitializationError> {
    // having an error during window creation triggers pre_window drop
    let window_attributes = Window::default_attributes()
      .with_title(WINDOW_TITLE)
      .with_inner_size(PhysicalSize {
        width: INITIAL_WINDOW_WIDTH,
        height: INITIAL_WINDOW_HEIGHT,
      })
      .with_min_inner_size(PhysicalSize {
        width: Ferris::WIDTH,
        height: Ferris::HEIGHT,
      });
    // .with_resizable(false)
    let window = event_loop.create_window(window_attributes)?;

    let mut destructor: Destructor<17> = Destructor::new();

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
      event_loop.display_handle()?,
      window.window_handle()?,
    )
    .on_err(|_| destroy_instance())?;
    destructor.push(&surface);

    // can return an error and can also return no devices
    let physical_device_creation = match unsafe {
      PhysicalDevice::select(&instance, &surface, initialization::select_physical_device)
    }
    .on_err(|_| destroy_instance())?
    {
      Some(tu) => tu,
      None => {
        destroy_instance();
        return Err(InitializationError::NoCompatibleDevices);
      }
    };

    let (device, queues) = Device::create(
      &instance,
      &physical_device_creation,
      DeviceExtensions {
        swapchain: true,
        ..Default::default()
      },
      DeviceExtensions {
        memory_priority: true,
        pageable_device_local_memory: true,
        swapchain_maintenance1: true,
        ..Default::default()
      },
      DeviceFeatures {
        synchronization2: true,
        dynamic_rendering: true,
        ..Default::default()
      },
      DeviceFeatures {
        swapchain_maintenance1: true,
        ..Default::default()
      },
    )
    .on_err(|_| destroy_instance())?;
    destructor.push(&device);

    let physical_device = physical_device_creation.physical_device;

    #[cfg(feature = "vl")]
    let debug_utils_marker = vkinitialization::DebugUtilsMarker::new(&instance, &device);
    #[cfg(feature = "vl")]
    unsafe {
      debug_utils_marker.set_queue_labels(queues);
    }

    let swapchains = Swapchains::new(
      &instance,
      &physical_device,
      &device,
      0,
      &surface,
      window.inner_size(),
      SWAPCHAIN_IMAGE_USAGES,
      #[cfg(feature = "vl")]
      &debug_utils_marker,
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&swapchains);

    let swapchain_format = swapchains.get_format();
    let texture_format = if KNOWN_FORMATS.contains(&swapchain_format) {
      swapchain_format
    } else {
      KNOWN_FORMATS
        .into_iter()
        .find(|&f| initialization::format_is_supported(&instance, *physical_device, f))
        .unwrap()
    };

    format_conversions::convert_rgba_data_to_format(&mut sprite_data.bytes, texture_format);
    log::info!("Creating texture with the format {:?}", texture_format);

    let shaper = font::SHAPER_DATA.shaper(&font::FONT_REF).build();
    let mut slug = SlugRendering::new(&font::FONT_FACE, shaper);

    slug.add_glyphs_in_str("0123456789");

    let mut text_vertices = Vec::new();
    let mut text_indices = Vec::new();
    let font_size = 60;
    let text = ["fps: 60", "Looks like it does", "support most fonts"];
    let mut full_size = slug.build_lines(
      &text,
      font_size,
      vk::Offset2D { x: 0, y: 0 },
      1.5,
      &mut text_vertices,
      &mut text_indices,
    );
    // todo: fix full slug size calculations
    full_size.total.max.x += 10.0;
    full_size.total.max.y += 20.0;

    let text_textures = slug.get_texture_data();

    let text_data = TextData {
      textures: text_textures,
      vertices: &text_vertices,
      indices: &text_indices,
    };

    let text_vertices_size =
      (text_data.vertices.len() * size_of::<crate::slug::SlugVertex>()) as u64;
    let text_indices_size = (text_data.indices.len() * size_of::<u32>()) as u64;

    let gpu_data = GPUData::new(
      &device,
      &physical_device,
      texture_format,
      sprite_data,
      &text_textures,
      TextBufferDimensions {
        device_vertices_size: text_vertices_size,
        device_indices_size: text_indices_size,
        cpu_vertices_size: text_vertices_size, // todo
        cpu_indices_size: text_indices_size,   // todo
      },
      full_size.total.to_vk_extent(),
      full_size,
      #[cfg(feature = "vl")]
      &debug_utils_marker,
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&gpu_data);

    // use same format for surface and the render target
    // see SWAPCHAIN_PREFERRED_IMAGE_FORMAT in render/mod.rs
    // vkCmdCopyImage does not convert formats, while vkCmdBlitImage does, so using different formats
    // would mean not using vkCmdCopyImage at all anymore
    let render_format = swapchains.get_format();

    let render_targets = RenderTargets::new(
      &device,
      &physical_device,
      render_format,
      #[cfg(feature = "vl")]
      &debug_utils_marker,
    )
    .on_err(|_| unsafe { destructor.fire(&device) })
    .map_err(|err| GPUDataAllocationError::from(err))?;
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

    let descriptor_pool =
      DescriptorPool::new(&device, &gpu_data).on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&descriptor_pool);

    log::debug!("Creating pipeline");
    let graphics_pipeline = GraphicsPipeline::new(
      &device,
      pipeline_cache,
      &descriptor_pool,
      render_format,
      RENDER_EXTENT,
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&graphics_pipeline);

    let text_pipeline = TextPipeline::new(
      &device,
      pipeline_cache,
      &descriptor_pool,
      render_format,
      RENDER_EXTENT,
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&text_pipeline);

    let graphics_pools = fill_destroyable_array_with_expression!(
      &device,
      GraphicsCommandBufferPool::create(
        &device,
        &physical_device.queue_families,
        #[cfg(feature = "vl")]
        &debug_utils_marker
      ),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(graphics_pools.as_ptr());

    let screenshot_buffer = ScreenshotBuffer::new(
      &device,
      &physical_device,
      #[cfg(feature = "vl")]
      &debug_utils_marker,
    )
    .on_err(|_| unsafe { destructor.fire(&device) })?;
    destructor.push(&screenshot_buffer);

    Ok(Self {
      window,
      surface,
      _entry: entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      #[cfg(feature = "vl")]
      debug_utils_marker,
      physical_device,
      device,
      queues,
      graphics_pools,
      data: gpu_data,
      pipeline: graphics_pipeline,
      pipeline_cache,
      swapchains,
      descriptor_pool,
      render_targets,
      screenshot_buffer,
      text_pipeline,
      // device_copy_pool,
      slug,
      text_vertices,
      text_indices,
    })
  }

  pub unsafe fn full_record_upload_initial_staging(
    &self,
    frame_i: usize,
    sprite_texture_bytes: &[u8],
  ) -> Result<(), HostMemorySyncError> {
    let pool = &self.graphics_pools[frame_i];
    pool.begin_recording(&self.device)?;
    self
      .data
      .write_and_record_initial_staging_data(&self.device, sprite_texture_bytes, pool)?;
    pool.end_recording(&self.device)?;
    Ok(())
  }

  pub unsafe fn record_upload_device_text_data(
    &mut self,
    frame_i: usize,
  ) -> Result<(), HostMemorySyncError> {
    let pool = &self.graphics_pools[frame_i];

    let text_data = TextData {
      textures: self.slug.get_texture_data(),
      vertices: &self.text_vertices,
      indices: &self.text_indices,
    };
    self
      .data
      .write_and_record_full_device_text_data(&self.device, &text_data, pool)?;

    Ok(())
  }

  pub unsafe fn record_graphics(
    &self,
    frame_i: usize,
    image_i: usize,
    position: &RenderPosition,
    save_to_screenshot_buffer: bool,
    update_text_ui: bool,
    draw_text: bool,
  ) {
    let pool = &self.graphics_pools[frame_i];

    if update_text_ui {
      pool.record_update_text_ui(
        &self.device,
        &self.descriptor_pool,
        &self.data,
        &self.text_pipeline,
      );
    }

    pool.record_main(
      frame_i,
      &self.device,
      &self.render_targets,
      self.swapchains.get_images()[image_i],
      self.swapchains.get_extent(),
      &self.pipeline,
      &self.descriptor_pool,
      &self.data,
      position,
      if save_to_screenshot_buffer {
        Some(*self.screenshot_buffer.buffer)
      } else {
        None
      },
      draw_text,
    );
  }

  pub unsafe fn recreate_swapchain(
    &mut self,
    cur_total_frame: usize,
  ) -> Result<(), SwapchainRecreationError> {
    // most of this function is just cleanup in case of an error

    // it is possible to use more than two frames in flight, but it would require having more than one old swapchain and pipeline
    #[allow(clippy::assertions_on_constants)]
    {
      assert!(FRAMES_IN_FLIGHT == 2);
    }

    // old swapchain becomes retired
    let (changes, destroyed_old) = self.swapchains.recreate(
      &self.physical_device,
      &self.device,
      cur_total_frame,
      &self.surface,
      self.window.inner_size(),
      SWAPCHAIN_IMAGE_USAGES,
      #[cfg(feature = "vl")]
      &self.debug_utils_marker,
    )?;

    if destroyed_old {
      self.cleanup_after_old_swapchain(cur_total_frame);
    }

    // shouldn't happen commonly
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
      let new_render_targets = RenderTargets::new(
        &self.device,
        &self.physical_device,
        new_format,
        #[cfg(feature = "vl")]
        &self.debug_utils_marker,
      )
      .on_err(|_| self.swapchains.revert_recreate(&self.device))?;

      log::info!("[Frame {}] Recreating pipeline", cur_total_frame);
      match self.pipeline.recreate(
        &self.device,
        self.pipeline_cache,
        self.swapchains.get_format(),
        RENDER_EXTENT,
      ) {
        Ok(v) => v,
        Err(err) => unsafe {
          new_render_targets.destroy_self(&self.device);
          self.swapchains.revert_recreate(&self.device);

          return Err(err.into());
        },
      }

      self.render_targets.destroy_self(&self.device);
      self.render_targets = new_render_targets;
    } else if !changes.extent {
      log::warn!(
        "[Frame {}] Recreating swapchain without any extent or format change",
        cur_total_frame
      );
    }

    Ok(())
  }

  // destroy old objects that resulted of a swapchain recreation
  // this should only be called when they stop being in use
  pub unsafe fn cleanup_after_old_swapchain(&mut self, cur_total_frame: usize) {
    self.pipeline.destroy_old(&self.device, cur_total_frame);
  }

  pub fn render_format(&self) -> vk::Format {
    self.swapchains.get_format()
  }

  // safety: screenshot buffer should not be in use
  pub fn save_screenshot_buffer_as_rgba8(
    &self,
    saved_format: vk::Format,
  ) -> Result<(), ImageError> {
    let mut data = unsafe { self.screenshot_buffer.read_memory(&self.device) }?;

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

      self.cleanup_after_old_swapchain(usize::MAX);

      log::info!("Saving pipeline cache");
      if let Err(err) =
        pipelines::save_pipeline_cache(&self.device, &self.physical_device, self.pipeline_cache)
      {
        log::error!("Failed to save pipeline cache: {:?}", err);
      }

      self.screenshot_buffer.destroy_self(&self.device);

      self.graphics_pools.destroy_self(&self.device);
      // self.device_copy_pool.destroy_self(&self.device);

      self.text_pipeline.destroy_self(&self.device);
      self.pipeline.destroy_self(&self.device);
      self.pipeline_cache.destroy_self(&self.device);
      self.descriptor_pool.destroy_self(&self.device);

      self.data.destroy_self(&self.device);

      self.render_targets.destroy_self(&self.device);
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
