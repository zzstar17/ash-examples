use ash::vk;
use std::{marker::PhantomData, ptr};
use vkinitialization::{
  device::{Device, DeviceExtensions, DeviceFeatures, PhysicalDevice, SingleQueues},
  InstanceOptionalExtensions,
};
use vkobjects::{
  destroy,
  errors::{OutOfMemoryError, QueueSubmitError},
  utility::OnErr,
  DeviceManuallyDestroyed, ManuallyDestroyed,
};

use crate::{
  command_pools::CommandPools,
  create_objs::{create_fence, create_semaphore},
  errors::InitializationError,
  gpu_data::GPUData,
  initialization,
  pipelines::{self, GraphicsPipeline},
  render_pass::create_render_pass,
};

pub struct Renderer {
  _entry: ash::Entry,
  instance: ash::Instance,
  #[cfg(feature = "vl")]
  debug_utils: vkinitialization::DebugUtils,
  #[cfg(feature = "vl")]
  marker: vkinitialization::DebugUtilsMarker,
  physical_device: PhysicalDevice,
  device: Device,
  queues: SingleQueues,

  command_pools: CommandPools,

  render_pass: vk::RenderPass,
  pipeline: GraphicsPipeline,

  gpu_data: GPUData,
}

impl Renderer {
  pub fn initialize(
    image_width: u32,
    image_height: u32,
    buffer_size: u64,
  ) -> Result<Self, InitializationError> {
    let entry: ash::Entry = unsafe { vkinitialization::get_entry() };

    let app_info = crate::initialization::get_app_info();
    #[cfg(feature = "vl")]
    let (instance, enabled_optional_extensions, debug_utils) =
      vkinitialization::create_instance(&entry, app_info, InstanceOptionalExtensions::default())?;
    #[cfg(not(feature = "vl"))]
    let (instance, enabled_optional_extensions) =
      vkinitialization::create_instance(&entry, app_info, InstanceOptionalExtensions::default())?;

    log::info!(
      "Enabled instance extensions:\n{:#?}",
      enabled_optional_extensions
    );

    let destroy_instance = || unsafe {
      #[cfg(feature = "vl")]
      destroy!(&debug_utils);
      destroy!(&instance);
    };

    // can return an error and can also return no devices
    let physical_device_creation =
      match unsafe { PhysicalDevice::select(&instance, initialization::select_physical_device) }
        .on_err(|_| destroy_instance())?
      {
        Some(tu) => tu,
        None => {
          destroy_instance();
          return Err(InitializationError::NoCompatibleDevices);
        }
      };

    // PhysicalDeviceFeatures::default();
    let (device, queues) = Device::create(
      &instance,
      &physical_device_creation,
      DeviceExtensions::default(),
      DeviceExtensions {
        memory_priority: true,
        pageable_device_local_memory: true,
        ..Default::default()
      },
      DeviceFeatures {
        synchronization2: true,
        ..Default::default()
      },
      DeviceFeatures::default(),
    )
    .on_err(|_| destroy_instance())?;

    let physical_device = physical_device_creation.physical_device;

    #[cfg(feature = "vl")]
    let marker = vkinitialization::DebugUtilsMarker::new(&instance, &device);
    #[cfg(feature = "vl")]
    unsafe {
      marker.set_queue_labels(queues);
    }

    let render_pass = create_render_pass(&device).on_err(|_| unsafe {
      destroy!(&device);
      destroy_instance();
    })?;

    let (gpu_data, gpu_data_pending_initialization) = GPUData::new(
      &device,
      &physical_device,
      render_pass,
      vk::Extent2D {
        width: image_width,
        height: image_height,
      },
      buffer_size,
      &queues,
      #[cfg(feature = "vl")]
      &marker,
    )
    .on_err(|_| unsafe {
      destroy!(&device => &render_pass, &device);
      destroy_instance();
    })?;

    log::info!("Creating pipeline cache");
    let (pipeline_cache, created_from_file) =
      pipelines::create_pipeline_cache(&device, &physical_device).on_err(|_| unsafe {
        destroy!(&device => &gpu_data_pending_initialization, &gpu_data, &render_pass, &device);
        destroy_instance();
      })?;
    if created_from_file {
      log::info!("Cache successfully created from an existing cache file");
    } else {
      log::info!("Cache initialized as empty");
    }

    log::debug!("Creating pipeline");
    let pipeline =
      GraphicsPipeline::create(&device, pipeline_cache, render_pass).on_err(|_| unsafe {
        destroy!(&device => &pipeline_cache, &gpu_data_pending_initialization, &gpu_data, &render_pass, &device);
        destroy_instance();
      })?;

    // no more pipelines will be created, so might as well save and delete the cache
    log::info!("Saving pipeline cache");
    if let Err(err) = pipelines::save_pipeline_cache(&device, &physical_device, pipeline_cache) {
      log::error!("Failed to save pipeline cache: {:?}", err);
    }
    unsafe {
      pipeline_cache.destroy_self(&device);
    }

    let command_pools = CommandPools::new(&device, &physical_device,#[cfg(feature = "vl")]
    &marker,).on_err(|_| unsafe {
      destroy!(&device => &pipeline, &gpu_data_pending_initialization, &gpu_data_pending_initialization, &gpu_data, &render_pass, &device);
      destroy_instance();
    })?;

    unsafe {
      gpu_data_pending_initialization
        .wait_and_self_destroy(&device)
        .on_err(|_| {
          destroy!(&device => &command_pools, &pipeline, &gpu_data, &render_pass, &device);
          destroy_instance();
        })?;
    }

    Ok(Self {
      _entry: entry,
      instance,
      #[cfg(feature = "vl")]
      debug_utils,
      #[cfg(feature = "vl")]
      marker,
      physical_device,
      device,
      queues,
      command_pools,
      gpu_data,
      render_pass,
      pipeline,
    })
  }

  pub unsafe fn record_work(&mut self) -> Result<(), OutOfMemoryError> {
    self.command_pools.graphics_pool.reset(&self.device)?;
    self.command_pools.graphics_pool.record_triangle(
      &self.device,
      &self.physical_device.queue_families,
      self.render_pass,
      &self.pipeline,
      &self.gpu_data,
    )?;

    self.command_pools.transfer_pool.reset(&self.device)?;
    self.command_pools.transfer_pool.record_copy_img_to_buffer(
      &self.device,
      &self.physical_device.queue_families,
      self.gpu_data.render_target,
      self.gpu_data.host_output_buffer,
    )?;

    Ok(())
  }

  // can return vk::Result::ERROR_DEVICE_LOST
  pub fn submit_and_wait(&self) -> Result<(), QueueSubmitError> {
    let image_clear_finished = create_semaphore(
      &self.device,
      #[cfg(feature = "vl")]
      &self.marker,
      #[cfg(feature = "vl")]
      c"image_clear_finished",
    )?;
    let all_done = create_fence(
      &self.device,
      #[cfg(feature = "vl")]
      &self.marker,
      #[cfg(feature = "vl")]
      c"all_done",
    )
    .on_err(|_| unsafe { destroy!(&self.device => &image_clear_finished) })?;

    let clear_image_submit = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 0,
      p_wait_semaphores: ptr::null(),
      p_wait_dst_stage_mask: ptr::null(),
      command_buffer_count: 1,
      p_command_buffers: &self.command_pools.graphics_pool.triangle,
      signal_semaphore_count: 1,
      p_signal_semaphores: &image_clear_finished,
      _marker: PhantomData,
    };
    let wait_for = vk::PipelineStageFlags::TRANSFER;
    let transfer_image_submit = vk::SubmitInfo {
      s_type: vk::StructureType::SUBMIT_INFO,
      p_next: ptr::null(),
      wait_semaphore_count: 1,
      p_wait_semaphores: &image_clear_finished,
      p_wait_dst_stage_mask: &wait_for,
      command_buffer_count: 1,
      p_command_buffers: &self.command_pools.transfer_pool.copy_image_to_buffer,
      signal_semaphore_count: 0,
      p_signal_semaphores: ptr::null(),
      _marker: PhantomData,
    };

    let destroy_objs = || unsafe { destroy!(&self.device => &image_clear_finished, &all_done) };

    unsafe {
      self
        .device
        .queue_submit(
          self.queues.graphics.handle,
          &[clear_image_submit],
          vk::Fence::null(),
        )
        .on_err(|_| destroy_objs())?;
      self
        .device
        .queue_submit(
          self.queues.transfer.handle,
          &[transfer_image_submit],
          all_done,
        )
        .on_err(|_| destroy_objs())?;

      self
        .device
        .wait_for_fences(&[all_done], true, u64::MAX)
        .on_err(|_| destroy_objs())?;
    }

    destroy_objs();

    Ok(())
  }

  pub unsafe fn get_resulting_data(&self, buffer_size: u64) -> Result<&[u8], vk::Result> {
    self
      .gpu_data
      .map_buffer_after_completion(&self.device, &self.physical_device, buffer_size)
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

      destroy!(&self.device => &self.command_pools, &self.gpu_data, &self.pipeline, &self.render_pass);

      ManuallyDestroyed::destroy_self(&self.device);

      #[cfg(feature = "vl")]
      {
        ManuallyDestroyed::destroy_self(&self.debug_utils);
      }
      ManuallyDestroyed::destroy_self(&self.instance);
    }
  }
}
