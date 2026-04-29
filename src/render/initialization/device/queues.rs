use std::{marker::PhantomData, ops::Deref, ptr};

use ash::vk;

use crate::render::initialization::{Surface, SurfaceError};

#[derive(Debug, Clone, Copy)]
pub struct QueueFamily {
  pub index: u32,
  pub queue_count: u32,
}

impl PartialEq for QueueFamily {
  fn eq(&self, other: &Self) -> bool {
    self.index == other.index
  }
}

#[derive(Debug, Clone, Copy)]
pub struct Queue {
  pub handle: vk::Queue,
  pub family_index: u32,
  pub index_in_family: u32,
}

impl Deref for Queue {
  type Target = vk::Queue;

  fn deref(&self) -> &Self::Target {
    &self.handle
  }
}

#[derive(Debug)]
pub struct QueueFamilies {
  pub graphics: QueueFamily,
  pub transfer: Option<QueueFamily>,
}

#[derive(Debug, thiserror::Error)]
pub enum QueueFamilyError {
  #[error("Surface error")]
  SurfaceError(#[from] SurfaceError),
  #[error("Device does not support required queue families or surface capabilities")]
  DoesNotSupportRequiredQueueFamilies,
}

// unsupported specialized queues get substituted by a more general supported counterpart
#[derive(Debug, Clone, Copy)]
pub struct SingleQueues {
  pub graphics: Queue,
  pub transfer: Queue,
}

fn queue_create_info<'a>(
  index: u32,
  count: u32,
  priorities_ptr: *const f32,
) -> vk::DeviceQueueCreateInfo<'a> {
  vk::DeviceQueueCreateInfo {
    s_type: vk::StructureType::DEVICE_QUEUE_CREATE_INFO,
    queue_family_index: index,
    queue_count: count,
    p_queue_priorities: priorities_ptr,
    p_next: ptr::null(),
    flags: vk::DeviceQueueCreateFlags::empty(),
    _marker: PhantomData,
  }
}

static SINGLE_QUEUE_PRIORITIES: [f32; QueueFamilies::FAMILY_COUNT] =
  [0.5; QueueFamilies::FAMILY_COUNT];

pub fn get_single_queue_create_infos(
  queue_families: &QueueFamilies,
) -> (
  [vk::DeviceQueueCreateInfo<'_>; QueueFamilies::FAMILY_COUNT],
  usize,
) {
  let mut total_unique_queues = 1;
  let mut c_infos = [vk::DeviceQueueCreateInfo::default(); QueueFamilies::FAMILY_COUNT];

  c_infos[0] = queue_create_info(
    queue_families.graphics.index,
    1,
    SINGLE_QUEUE_PRIORITIES.as_ptr(),
  );

  match queue_families.transfer {
    Some(f) => {
      c_infos[total_unique_queues] =
        queue_create_info(f.index, 1, SINGLE_QUEUE_PRIORITIES.as_ptr());
      total_unique_queues += 1;
    }
    None => {
      if c_infos[0].queue_count + 1 < queue_families.graphics.queue_count {
        c_infos[0].queue_count += 1;
      }
    }
  }

  (c_infos, total_unique_queues)
}

pub unsafe fn retrieve_single_queues(
  device: &ash::Device,
  queue_families: &QueueFamilies,
  c_infos: &[vk::DeviceQueueCreateInfo],
) -> SingleQueues {
  let graphics = Queue {
    handle: device.get_device_queue(queue_families.graphics.index, 0),
    family_index: queue_families.graphics.index,
    index_in_family: 0,
  };

  // #[cfg(all(not(feature = "graphics_family"), feature = "compute_family"))]
  let mut non_specialized_i = 1;
  let mut next_non_specialized_queue = || {
    if non_specialized_i < c_infos[0].queue_count {
      let r = Some(Queue {
        handle: device.get_device_queue(c_infos[0].queue_family_index, non_specialized_i),
        family_index: c_infos[0].queue_family_index,
        index_in_family: non_specialized_i,
      });
      non_specialized_i += 1;
      r
    } else {
      None
    }
  };

  let transfer = if let Some(transfer_f) = queue_families.transfer {
    Queue {
      handle: device.get_device_queue(transfer_f.index, 0),
      family_index: transfer_f.index,
      index_in_family: 0,
    }
  } else {
    let default = graphics;
    next_non_specialized_queue().unwrap_or(default)
  };

  SingleQueues { graphics, transfer }
}

impl QueueFamilies {
  pub const FAMILY_COUNT: usize = 2;

  pub fn get_from_physical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
    surface: &Surface,
  ) -> Result<Self, QueueFamilyError> {
    let properties =
      unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let mut graphics = None;
    let mut transfer = None; // non graphics
    for (i, props) in properties.into_iter().enumerate() {
      let family = Some(QueueFamily {
        index: i as u32,
        queue_count: props.queue_count,
      });
      if props.queue_flags.contains(vk::QueueFlags::GRAPHICS)
        && graphics.is_none()
        && unsafe { surface.supports_queue_family(physical_device, i)? }
      {
        graphics = family;
        continue;
      }
      if props.queue_flags.contains(vk::QueueFlags::TRANSFER) && transfer.is_none() {
        transfer = family;
        continue;
      }
    }

    if graphics.is_none() {
      return Err(QueueFamilyError::DoesNotSupportRequiredQueueFamilies);
    }
    Ok(QueueFamilies {
      graphics: graphics.unwrap(),
      transfer,
    })
  }
}
