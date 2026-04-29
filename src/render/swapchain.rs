use std::{marker::PhantomData, mem, ops::Deref, ptr};

pub use ash::vk;
use winit::dpi::PhysicalSize;

use crate::{utility::OnErr, PREFERRED_PRESENTATION_METHOD};

use super::{
  create_objs::create_image_view,
  device_destroyable::DeviceManuallyDestroyed,
  errors::{error_chain_fmt, OutOfMemoryError},
  initialization::{
    device::{PhysicalDevice, QueueFamilies},
    Surface, SurfaceError,
  },
};

// VK_ERROR_NATIVE_WINDOW_IN_USE_KHR shouldn't happen unless some other program somehow hijacks
//    the created window other API
// VK_ERROR_COMPRESSION_EXHAUSTED_EXT shouldn't happen because there are no compressed image that
//    are requested
#[derive(thiserror::Error)]
pub enum SwapchainCreationError {
  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),

  #[error("Device is lost (https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#devsandqueues-lost-device)")]
  DeviceIsLost,
  #[error("Surface is lost and no longer available")]
  SurfaceIsLost,
  #[error("Creation failed because of some other error")]
  GenericInitializationError,
}
impl std::fmt::Debug for SwapchainCreationError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

// VK_ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT needs VK_FULL_SCREEN_EXCLUSIVE_APPLICATION_CONTROLLED_EXT
#[derive(thiserror::Error)]
pub enum AcquireNextImageError {
  #[error("Swapchain is out of date and needs to be recreated")]
  OutOfDate,

  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),

  #[error("Device is lost (https://registry.khronos.org/vulkan/specs/1.3-extensions/html/vkspec.html#devsandqueues-lost-device)")]
  DeviceIsLost,
  #[error("Surface is lost and no longer available")]
  SurfaceIsLost,
}
impl std::fmt::Debug for AcquireNextImageError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    error_chain_fmt(self, f)
  }
}

impl From<vk::Result> for SwapchainCreationError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        SwapchainCreationError::OutOfMemory(value.into())
      }
      vk::Result::ERROR_DEVICE_LOST => SwapchainCreationError::DeviceIsLost,
      vk::Result::ERROR_SURFACE_LOST_KHR => SwapchainCreationError::SurfaceIsLost,
      vk::Result::ERROR_INITIALIZATION_FAILED => SwapchainCreationError::GenericInitializationError,

      vk::Result::ERROR_NATIVE_WINDOW_IN_USE_KHR => {
        panic!("Swapchain creation returned VK_ERROR_NATIVE_WINDOW_IN_USE_KHR")
      }
      vk::Result::ERROR_COMPRESSION_EXHAUSTED_EXT => {
        panic!("Swapchain creation returned VK_ERROR_COMPRESSION_EXHAUSTED_EXT")
      }
      _ => panic!(),
    }
  }
}

impl From<vk::Result> for AcquireNextImageError {
  fn from(value: vk::Result) -> Self {
    match value {
      vk::Result::ERROR_OUT_OF_DATE_KHR => AcquireNextImageError::OutOfDate,
      vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
        AcquireNextImageError::OutOfMemory(value.into())
      }
      vk::Result::ERROR_DEVICE_LOST => AcquireNextImageError::DeviceIsLost,
      vk::Result::ERROR_SURFACE_LOST_KHR => AcquireNextImageError::SurfaceIsLost,
      vk::Result::ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT => {
        panic!("Acquire next image returned VK_ERROR_FULL_SCREEN_EXCLUSIVE_MODE_LOST_EXT")
      }
      _ => panic!(),
    }
  }
}

impl From<OutOfMemoryError> for SwapchainCreationError {
  fn from(value: OutOfMemoryError) -> Self {
    SwapchainCreationError::OutOfMemory(value)
  }
}

impl From<SurfaceError> for SwapchainCreationError {
  fn from(value: SurfaceError) -> Self {
    match value {
      SurfaceError::OutOfMemory(err) => SwapchainCreationError::OutOfMemory(err),
      SurfaceError::SurfaceIsLost => SwapchainCreationError::SurfaceIsLost,
    }
  }
}

pub struct Swapchains {
  loader: ash::khr::swapchain::Device,
  current: Swapchain,
  old: Option<Swapchain>,
}

impl Swapchains {
  pub fn new(
    instance: &ash::Instance,
    physical_device: &PhysicalDevice,
    device: &ash::Device,
    surface: &Surface,
    window_size: PhysicalSize<u32>,
    image_usages: vk::ImageUsageFlags,
  ) -> Result<Self, SwapchainCreationError> {
    let loader = ash::khr::swapchain::Device::new(instance, device);

    let current = Swapchain::create(
      physical_device,
      device,
      surface,
      &loader,
      window_size,
      image_usages,
    )?;

    Ok(Self {
      loader,
      current,
      old: None,
    })
  }

  pub unsafe fn acquire_next_image(
    &mut self,
    semaphore: vk::Semaphore,
  ) -> Result<(u32, bool), AcquireNextImageError> {
    self
      .current
      .acquire_next_image(semaphore, &self.loader)
      .map_err(AcquireNextImageError::from)
  }

  pub unsafe fn recreate(
    &mut self,
    physical_device: &PhysicalDevice,
    device: &ash::Device,
    surface: &Surface,
    window_size: PhysicalSize<u32>,
    image_usages: vk::ImageUsageFlags,
  ) -> Result<RecreationChanges, SwapchainCreationError> {
    let (old, changes) = self.current.recreate(
      physical_device,
      device,
      surface,
      &self.loader,
      window_size,
      image_usages,
    )?;

    self.old = Some(old);
    Ok(changes)
  }

  pub fn revert_recreate(&mut self, device: &ash::Device) {
    unsafe {
      self.current.destroy_self(&self.loader, device);
    }
    let mut temp = None;
    mem::swap(&mut self.old, &mut temp);
    self.current = temp.unwrap();
  }

  pub unsafe fn queue_present(
    &mut self,
    image_index: u32,
    present_queue: vk::Queue,
    wait_semaphores: &[vk::Semaphore],
  ) -> Result<bool, AcquireNextImageError> {
    let present_info = vk::PresentInfoKHR {
      s_type: vk::StructureType::PRESENT_INFO_KHR,
      p_next: ptr::null(),
      wait_semaphore_count: wait_semaphores.len() as u32,
      p_wait_semaphores: wait_semaphores.as_ptr(),
      swapchain_count: 1,
      p_swapchains: &*self.current,
      p_image_indices: &image_index,
      p_results: ptr::null_mut(),
      _marker: PhantomData,
    };

    unsafe { self.loader.queue_present(present_queue, &present_info) }
      .map_err(AcquireNextImageError::from)
  }

  pub fn destroy_old(&mut self, device: &ash::Device) {
    if let Some(old) = &mut self.old {
      unsafe {
        old.destroy_self(&self.loader, device);
      }
      self.old = None;
    }
  }

  pub fn get_format(&self) -> vk::Format {
    self.current.format
  }

  pub fn get_extent(&self) -> vk::Extent2D {
    self.current.extent
  }

  pub fn get_image_views(&self) -> &[vk::ImageView] {
    &self.current.image_views
  }

  pub fn get_images(&self) -> &[vk::Image] {
    &self.current.images
  }
}

impl DeviceManuallyDestroyed for Swapchains {
  unsafe fn destroy_self(&self, device: &ash::Device) {
    if let Some(old) = &self.old {
      old.destroy_self(&self.loader, device);
    }
    self.current.destroy_self(&self.loader, device);
  }
}

#[derive(Debug)]
struct Swapchain {
  inner: vk::SwapchainKHR,
  images: Box<[vk::Image]>, // owned by the swapchain
  pub image_views: Box<[vk::ImageView]>,
  pub format: vk::Format,
  pub extent: vk::Extent2D,
}

impl Deref for Swapchain {
  type Target = vk::SwapchainKHR;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

pub struct RecreationChanges {
  pub format: bool,
  pub extent: bool,
}

impl Swapchain {
  pub fn create(
    physical_device: &PhysicalDevice,
    device: &ash::Device,
    surface: &Surface,
    swapchain_loader: &ash::khr::swapchain::Device,
    window_size: PhysicalSize<u32>,
    image_usages: vk::ImageUsageFlags,
  ) -> Result<Self, SwapchainCreationError> {
    let capabilities = unsafe { surface.get_capabilities(**physical_device) }?;
    let image_format = select_swapchain_image_format(**physical_device, surface)?;
    let present_mode = select_swapchain_present_mode(**physical_device, surface)?;
    let extent = get_swapchain_extent(&capabilities, window_size);

    log::info!(
      "Creating swapchain with ({}, {}) extent, {:?} format and {:?} present mode",
      extent.width,
      extent.height,
      image_format,
      present_mode
    );

    Self::create_with(
      device,
      &physical_device.queue_families,
      surface,
      swapchain_loader,
      capabilities,
      image_format,
      image_usages,
      present_mode,
      extent,
      vk::SwapchainKHR::null(),
    )
  }

  pub fn recreate(
    &mut self,
    physical_device: &PhysicalDevice,
    device: &ash::Device,
    surface: &Surface,
    swapchain_loader: &ash::khr::swapchain::Device,
    window_size: PhysicalSize<u32>,
    image_usages: vk::ImageUsageFlags,
  ) -> Result<(Self, RecreationChanges), SwapchainCreationError> {
    let capabilities = unsafe { surface.get_capabilities(**physical_device) }?;
    let image_format = select_swapchain_image_format(**physical_device, surface)?;
    let present_mode = select_swapchain_present_mode(**physical_device, surface)?;
    let extent = get_swapchain_extent(&capabilities, window_size);

    log::info!(
      "Recreating swapchain with ({}, {}) extent, {:?} format and {:?} present mode",
      extent.width,
      extent.height,
      image_format,
      present_mode
    );

    let changes = RecreationChanges {
      format: image_format.format != self.format,
      extent: extent != self.extent,
    };

    let mut new = Self::create_with(
      device,
      &physical_device.queue_families,
      surface,
      swapchain_loader,
      capabilities,
      image_format,
      image_usages,
      present_mode,
      extent,
      self.inner,
    )?;

    let old = {
      std::mem::swap(&mut new, self);
      new
    };

    Ok((old, changes))
  }

  fn create_with(
    device: &ash::Device,
    queue_families: &QueueFamilies,
    surface: &Surface,
    swapchain_loader: &ash::khr::swapchain::Device,
    capabilities: vk::SurfaceCapabilitiesKHR,
    image_format: vk::SurfaceFormatKHR,
    image_usages: vk::ImageUsageFlags,
    present_mode: vk::PresentModeKHR,
    extent: vk::Extent2D,
    old_swapchain: vk::SwapchainKHR,
  ) -> Result<Self, SwapchainCreationError> {
    // it is usually recommended to use one more than the minimum number of images
    let image_count = if capabilities.max_image_count > 0 {
      (capabilities.min_image_count + 1).min(capabilities.max_image_count)
    } else {
      capabilities.min_image_count + 1
    };

    let mut create_info = vk::SwapchainCreateInfoKHR {
      s_type: vk::StructureType::SWAPCHAIN_CREATE_INFO_KHR,
      p_next: ptr::null(),
      flags: vk::SwapchainCreateFlagsKHR::empty(),
      surface: **surface,

      min_image_count: image_count,
      image_color_space: image_format.color_space,
      image_format: image_format.format,
      image_extent: extent,
      image_array_layers: 1,
      image_usage: image_usages,

      image_sharing_mode: vk::SharingMode::EXCLUSIVE,
      // ignored when SharingMode is EXCLUSIVE
      p_queue_family_indices: ptr::null(),
      queue_family_index_count: 0,

      pre_transform: capabilities.current_transform,
      composite_alpha: vk::CompositeAlphaFlagsKHR::OPAQUE,
      present_mode,
      clipped: vk::TRUE,
      old_swapchain,
      _marker: PhantomData,
    };

    let swapchain = unsafe { swapchain_loader.create_swapchain(&create_info, None) }?;

    let images = unsafe { swapchain_loader.get_swapchain_images(swapchain) }
      .map_err(OutOfMemoryError::from)
      .on_err(|_| unsafe { swapchain_loader.destroy_swapchain(swapchain, None) })?
      .into_boxed_slice();

    let image_views = {
      let mut image_views: Vec<vk::ImageView> = Vec::with_capacity(images.len());
      for &image in images.iter() {
        image_views.push(
          match create_image_view(device, image, image_format.format) {
            Ok(view) => view,
            Err(err) => unsafe {
              for view in image_views {
                view.destroy_self(device);
              }
              swapchain_loader.destroy_swapchain(swapchain, None);
              return Err(err.into());
            },
          },
        )
      }

      image_views.into_boxed_slice()
    };

    log::debug!(
      "Created swapchain with\nimages: {:?}\nimage views: {:?}",
      images,
      image_views
    );

    Ok(Self {
      inner: swapchain,
      images,
      image_views,
      format: image_format.format,
      extent,
    })
  }

  pub unsafe fn acquire_next_image(
    &mut self,
    semaphore: vk::Semaphore,
    loader: &ash::khr::swapchain::Device,
  ) -> Result<(u32, bool), vk::Result> {
    loader.acquire_next_image(self.inner, u64::MAX, semaphore, vk::Fence::null())
  }

  pub unsafe fn destroy_self(&self, loader: &ash::khr::swapchain::Device, device: &ash::Device) {
    for view in self.image_views.iter() {
      view.destroy_self(device);
    }
    loader.destroy_swapchain(self.inner, None);
  }
}

fn select_swapchain_image_format(
  physical_device: vk::PhysicalDevice,
  surface: &Surface,
) -> Result<vk::SurfaceFormatKHR, SurfaceError> {
  let formats = unsafe { surface.get_formats(physical_device) }?;
  for available_format in formats.iter() {
    // commonly available
    if available_format.format == vk::Format::B8G8R8A8_SRGB
      && available_format.color_space == vk::ColorSpaceKHR::SRGB_NONLINEAR
    {
      return Ok(*available_format);
    }
  }

  Ok(formats[0])
}

fn select_swapchain_present_mode(
  physical_device: vk::PhysicalDevice,
  surface: &Surface,
) -> Result<vk::PresentModeKHR, SurfaceError> {
  let present_modes = unsafe { surface.get_present_modes(physical_device) }?;
  if present_modes.contains(&PREFERRED_PRESENTATION_METHOD) {
    return Ok(PREFERRED_PRESENTATION_METHOD);
  }

  if PREFERRED_PRESENTATION_METHOD == vk::PresentModeKHR::FIFO_RELAXED
    && present_modes.contains(&vk::PresentModeKHR::IMMEDIATE)
  {
    return Ok(vk::PresentModeKHR::IMMEDIATE);
  }

  if PREFERRED_PRESENTATION_METHOD == vk::PresentModeKHR::IMMEDIATE
    && present_modes.contains(&vk::PresentModeKHR::MAILBOX)
  {
    return Ok(vk::PresentModeKHR::MAILBOX);
  }

  // required to be available
  Ok(vk::PresentModeKHR::FIFO)
}

fn get_swapchain_extent(
  capabilities: &vk::SurfaceCapabilitiesKHR,
  size: PhysicalSize<u32>,
) -> vk::Extent2D {
  match Surface::get_extent_from_capabilities(capabilities) {
    Some(extent) => extent,
    None => vk::Extent2D {
      width: size.width.clamp(
        capabilities.min_image_extent.width,
        capabilities.max_image_extent.width,
      ),
      height: size.height.clamp(
        capabilities.min_image_extent.height,
        capabilities.max_image_extent.height,
      ),
    },
  }
}
