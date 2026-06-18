use ash::vk;

use vkobjects::{
  errors::OutOfMemoryError, fill_destroyable_array_with_expression, utility::OnErr,
  DeviceManuallyDestroyed, ManuallyDestroyed,
};

use crate::{
  compute::{ParticleBuffers, ParticlesDraw},
  destructor::Destructor,
  render::{gpu_data::GPUDataAllocationError, PostWindowInit},
  RESOLUTION, SCREENSHOT_SAVE_FILE,
};

use super::{
  command_pools::GraphicsCommandBufferPool,
  descriptor_sets::DescriptorPool,
  errors::{ImageError, InitializationError, SwapchainRecreationError},
  format_conversions::{self, KNOWN_FORMATS},
  gpu_data::GPUData,
  initialization::{self},
  pipelines::{self, GraphicsPipeline},
  render_object::RenderPosition,
  render_pass::create_render_pass,
  render_targets::RenderTargets,
  screenshot_buffer::ScreenshotBuffer,
  swapchain::{SwapchainCreationError, Swapchains},
  FRAMES_IN_FLIGHT, RENDER_EXTENT, SWAPCHAIN_IMAGE_USAGES,
};

const TEXTURE_PATH: &str = "./ferris.png";

fn read_texture_bytes_as_rgba8() -> Result<(u32, u32, Vec<u8>), image::ImageError> {
  let img = image::ImageReader::open(TEXTURE_PATH)?
    .decode()?
    .into_rgba8();
  let width = img.width();
  let height = img.height();

  let bytes = img.into_raw();
  assert!(bytes.len() == width as usize * height as usize * 4);
  Ok((width, height, bytes))
}

pub struct Renderer {
  pub init: PostWindowInit,

  pub swapchains: Swapchains,

  render_pass: vk::RenderPass,
  render_targets: RenderTargets,

  pipeline_cache: vk::PipelineCache,
  pipeline: GraphicsPipeline,
  pub command_pools: [GraphicsCommandBufferPool; FRAMES_IN_FLIGHT],

  pub particle_buffers: ParticleBuffers, // not owned
  data: GPUData,
  descriptor_pool: DescriptorPool,

  screenshot_buffer: ScreenshotBuffer,
}

impl Renderer {
  pub fn initialize(
    post_window: PostWindowInit,
    particle_buffers: ParticleBuffers,
  ) -> Result<Self, InitializationError> {
    let mut destructor: Destructor<11> = Destructor::new();

    let swapchains = Swapchains::new(
      &post_window.instance,
      &post_window.physical_device,
      &post_window.device,
      0,
      &post_window.surface,
      post_window.window.inner_size(),
      SWAPCHAIN_IMAGE_USAGES,
      #[cfg(feature = "vl")]
      &post_window.debug_utils_marker,
    )
    .on_err(|_| unsafe {
      ManuallyDestroyed::destroy_self(&post_window);
    })?;
    destructor.push(&swapchains);

    let swapchain_format = swapchains.get_format();
    let texture_format = if KNOWN_FORMATS.contains(&swapchain_format) {
      swapchain_format
    } else {
      KNOWN_FORMATS
        .into_iter()
        .find(|&f| {
          initialization::format_is_supported(
            &post_window.instance,
            *post_window.physical_device,
            f,
          )
        })
        .unwrap()
    };

    let (width, height, mut texture_data) = read_texture_bytes_as_rgba8().on_err(|_| unsafe {
      destructor.fire(&post_window.device);
      ManuallyDestroyed::destroy_self(&post_window);
    })?;
    let texture_extent = vk::Extent2D { width, height };
    format_conversions::convert_rgba_data_to_format(&mut texture_data, texture_format);
    log::info!("Creating texture with the format {:?}", texture_format);

    let (gpu_data, gpu_data_pending_initialization) = GPUData::new(
      &post_window.device,
      &post_window.physical_device,
      texture_extent,
      texture_format,
      texture_data,
      &post_window.queues,
      #[cfg(feature = "vl")]
      &post_window.debug_utils_marker,
    )
    .on_err(|_| unsafe {
      destructor.fire(&post_window.device);
      ManuallyDestroyed::destroy_self(&post_window);
    })?;
    destructor.push(&gpu_data);
    destructor.push(&gpu_data_pending_initialization);

    // use same format for surface and the render target
    // see SWAPCHAIN_PREFERRED_IMAGE_FORMAT in render/mod.rs
    // vkCmdCopyImage does not convert formats, while vkCmdBlitImage does, so using different formats
    // would mean not using vkCmdCopyImage at all anymore
    let render_format = swapchains.get_format();
    let render_pass =
      create_render_pass(&post_window.device, render_format).on_err(|_| unsafe {
        destructor.fire(&post_window.device);
        ManuallyDestroyed::destroy_self(&post_window);
      })?;
    destructor.push(&render_pass);

    let render_targets = RenderTargets::new(
      &post_window.device,
      &post_window.physical_device,
      render_pass,
      render_format,
      #[cfg(feature = "vl")]
      &post_window.debug_utils_marker,
    )
    .on_err(|_| unsafe {
      destructor.fire(&post_window.device);
      ManuallyDestroyed::destroy_self(&post_window);
    })
    .map_err(|err| GPUDataAllocationError::from(err))?;
    log::debug!("Created render targets:\n{:#?}", render_targets);
    destructor.push(&render_targets);

    log::info!("Creating pipeline cache");
    let (pipeline_cache, created_from_file) =
      pipelines::create_pipeline_cache(&post_window.device, &post_window.physical_device).on_err(
        |_| unsafe {
          destructor.fire(&post_window.device);
          ManuallyDestroyed::destroy_self(&post_window);
        },
      )?;
    if created_from_file {
      log::info!("Cache successfully created from an existing cache file");
    } else {
      log::info!("Cache initialized as empty");
    }
    destructor.push(&pipeline_cache);

    let descriptor_pool =
      DescriptorPool::new(&post_window.device, gpu_data.texture_view).on_err(|_| unsafe {
        destructor.fire(&post_window.device);
        ManuallyDestroyed::destroy_self(&post_window);
      })?;
    destructor.push(&descriptor_pool);

    log::debug!("Creating pipeline");
    let graphics_pipeline = GraphicsPipeline::new(
      &post_window.device,
      pipeline_cache,
      render_pass,
      &descriptor_pool,
      RENDER_EXTENT,
    )
    .on_err(|_| unsafe {
      destructor.fire(&post_window.device);
      ManuallyDestroyed::destroy_self(&post_window);
    })?;
    destructor.push(&graphics_pipeline);

    let command_pools = fill_destroyable_array_with_expression!(
      &post_window.device,
      GraphicsCommandBufferPool::create(
        &post_window.device,
        &post_window.physical_device.queue_families,
        #[cfg(feature = "vl")]
        &post_window.debug_utils_marker
      ),
      FRAMES_IN_FLIGHT
    )
    .on_err(|_| unsafe {
      destructor.fire(&post_window.device);
      ManuallyDestroyed::destroy_self(&post_window);
    })?;
    destructor.push(command_pools.as_ptr());

    unsafe {
      gpu_data_pending_initialization
        .wait_and_self_destroy(&post_window.device)
        .on_err(|_| {
          destructor.fire(&post_window.device);
          ManuallyDestroyed::destroy_self(&post_window);
        })?;
    }
    let screenshot_buffer = ScreenshotBuffer::new(
      &post_window.device,
      &post_window.physical_device,
      #[cfg(feature = "vl")]
      &post_window.debug_utils_marker,
    )
    .on_err(|_| unsafe {
      destructor.fire(&post_window.device);
      ManuallyDestroyed::destroy_self(&post_window);
    })?;
    destructor.push(&screenshot_buffer);

    Ok(Self {
      init: post_window,
      command_pools,
      data: gpu_data,
      render_pass,
      pipeline: graphics_pipeline,
      pipeline_cache,
      swapchains,
      descriptor_pool,
      render_targets,
      screenshot_buffer,
      particle_buffers,
    })
  }

  pub unsafe fn record_graphics(
    &mut self,
    frame_i: usize,
    image_i: usize,
    position: &RenderPosition,
    particles_draw_opt: Option<ParticlesDraw>,
    save_to_screenshot_buffer: bool,
  ) -> Result<(), OutOfMemoryError> {
    self.command_pools[frame_i].reset(&self.init.device)?;
    self.command_pools[frame_i].record_main(
      frame_i,
      &self.init.device,
      &self.init.queues,
      self.render_pass,
      &self.render_targets,
      self.swapchains.get_images()[image_i],
      self.swapchains.get_extent(),
      &self.pipeline,
      &self.descriptor_pool,
      &self.data,
      particles_draw_opt,
      position,
      if save_to_screenshot_buffer {
        Some(*self.screenshot_buffer.buffer)
      } else {
        None
      },
    )?;
    Ok(())
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
      &self.init.physical_device,
      &self.init.device,
      cur_total_frame,
      &self.init.surface,
      self.init.window.inner_size(),
      SWAPCHAIN_IMAGE_USAGES,
      #[cfg(feature = "vl")]
      &self.init.debug_utils_marker,
    )?;

    if destroyed_old {
      self.cleanup_after_old_swapchain(cur_total_frame);
    }

    let mut new_render_pass = None;
    let mut new_render_targets = None;

    // shouldn't happen commonly
    if changes.format {
      log::info!("Changing swapchain format");

      // this shouldn't happen regularly, so its okay to stop all rendering so that the render pass can be recreated
      self
        .init
        .device
        .device_wait_idle()
        .on_err(|_| self.swapchains.revert_recreate(&self.init.device))
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
        create_render_pass(&self.init.device, new_format)
          .on_err(|_| self.swapchains.revert_recreate(&self.init.device))?,
      );
      new_render_targets = Some(
        RenderTargets::new(
          &self.init.device,
          &self.init.physical_device,
          new_render_pass.unwrap(),
          new_format,
          #[cfg(feature = "vl")]
          &self.init.debug_utils_marker,
        )
        .on_err(|_| {
          new_render_pass.unwrap().destroy_self(&self.init.device);
          self.swapchains.revert_recreate(&self.init.device)
        })?,
      );
    } else if !changes.extent {
      log::warn!(
        "[Frame {}] Recreating swapchain without any extent or format change",
        cur_total_frame
      );
    }

    // recreate pipeline because of a new render pass
    if changes.format {
      log::info!("[Frame {}] Recreating pipeline", cur_total_frame);
      match self.pipeline.recreate(
        &self.init.device,
        self.pipeline_cache,
        self.render_pass,
        RENDER_EXTENT,
      ) {
        Ok(v) => v,
        Err(err) => unsafe {
          if let Some(render_targets) = new_render_targets {
            render_targets.destroy_self(&self.init.device);
          }
          if let Some(render_pass) = new_render_pass {
            render_pass.destroy_self(&self.init.device);
          }
          self.swapchains.revert_recreate(&self.init.device);

          return Err(err.into());
        },
      }
    }

    if let Some(new) = new_render_pass {
      self.render_pass.destroy_self(&self.init.device);
      self.render_pass = new;
    }
    if let Some(new) = new_render_targets {
      self.render_targets.destroy_self(&self.init.device);
      self.render_targets = new;
    }

    Ok(())
  }

  // destroy old objects that resulted of a swapchain recreation
  // this should only be called when they stop being in use
  pub unsafe fn cleanup_after_old_swapchain(&mut self, cur_total_frame: usize) {
    self
      .pipeline
      .destroy_old(&self.init.device, cur_total_frame);
  }

  pub fn render_format(&self) -> vk::Format {
    self.swapchains.get_format()
  }

  // safety: screenshot buffer should not be in use
  pub fn save_screenshot_buffer_as_rgba8(
    &self,
    saved_format: vk::Format,
  ) -> Result<(), ImageError> {
    let mut data = unsafe { self.screenshot_buffer.read_memory(&self.init.device) }?;

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

  pub unsafe fn destroy_self(&mut self) {
    log::debug!("Destroying renderer objects...");

    self.cleanup_after_old_swapchain(usize::MAX);

    log::info!("Saving pipeline cache");
    if let Err(err) = pipelines::save_pipeline_cache(
      &self.init.device,
      &self.init.physical_device,
      self.pipeline_cache,
    ) {
      log::error!("Failed to save pipeline cache: {:?}", err);
    }

    let device = &self.init.device;

    self.screenshot_buffer.destroy_self(device);

    self.command_pools.destroy_self(device);

    self.pipeline.destroy_self(device);
    self.pipeline_cache.destroy_self(device);
    self.descriptor_pool.destroy_self(device);

    self.data.destroy_self(device);

    self.render_targets.destroy_self(device);
    self.render_pass.destroy_self(device);
    self.swapchains.destroy_self(device);

    ManuallyDestroyed::destroy_self(&self.init);
  }
}
