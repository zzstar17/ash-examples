use std::{marker::PhantomData, ops::Deref, ptr};

use ash::vk::{self};

#[derive(Debug, Copy, Clone)]
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
  #[cfg(feature = "graphics_family")]
  pub graphics: QueueFamily,

  #[cfg(all(feature = "graphics_family", feature = "compute_family"))]
  pub compute: Option<QueueFamily>,
  #[cfg(not(feature = "graphics_family"))]
  pub compute: QueueFamily,

  #[cfg(feature = "transfer_family")]
  pub transfer: Option<QueueFamily>,
}

// unsupported specialized queues get substituted by a more general supported counterpart
#[derive(Debug, Clone, Copy)]
pub struct SingleQueues {
  #[cfg(feature = "graphics_family")]
  pub graphics: Queue,
  #[cfg(feature = "compute_family")]
  pub compute: Queue,
  #[cfg(feature = "transfer_family")]
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

#[cfg(feature = "graphics_family")]
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

  #[cfg(feature = "compute_family")]
  match queue_families.compute {
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

  #[cfg(feature = "transfer_family")]
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

#[cfg(not(feature = "graphics_family"))]
pub fn get_single_queue_create_infos(
  queue_families: &QueueFamilies,
) -> (
  [vk::DeviceQueueCreateInfo<'_>; QueueFamilies::FAMILY_COUNT],
  usize,
) {
  let mut total_unique_queues = 1;
  let mut c_infos = [vk::DeviceQueueCreateInfo::default(); QueueFamilies::FAMILY_COUNT];

  c_infos[0] = queue_create_info(
    queue_families.compute.index,
    1,
    SINGLE_QUEUE_PRIORITIES.as_ptr(),
  );

  #[cfg(feature = "transfer_family")]
  match queue_families.transfer {
    Some(f) => {
      c_infos[total_unique_queues] =
        queue_create_info(f.index, 1, SINGLE_QUEUE_PRIORITIES.as_ptr());
      total_unique_queues += 1;
    }
    None => {
      if c_infos[0].queue_count + 1 < queue_families.compute.queue_count {
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
  #[cfg(feature = "graphics_family")]
  let graphics = Queue {
    handle: device.get_device_queue(queue_families.graphics.index, 0),
    family_index: queue_families.graphics.index,
    index_in_family: 0,
  };

  #[cfg(not(feature = "graphics_family"))]
  let compute = Queue {
    handle: device.get_device_queue(queue_families.compute.index, 0),
    family_index: queue_families.compute.index,
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

  #[cfg(all(feature = "graphics_family", feature = "compute_family"))]
  let compute = if let Some(compute_f) = queue_families.compute {
    Queue {
      handle: device.get_device_queue(compute_f.index, 0),
      family_index: compute_f.index,
      index_in_family: 0,
    }
  } else {
    next_non_specialized_queue().unwrap_or(graphics)
  };

  #[cfg(feature = "transfer_family")]
  let transfer = if let Some(transfer_f) = queue_families.transfer {
    Queue {
      handle: device.get_device_queue(transfer_f.index, 0),
      family_index: transfer_f.index,
      index_in_family: 0,
    }
  } else {
    #[cfg(feature = "graphics_family")]
    let default = graphics;
    #[cfg(not(feature = "graphics_family"))]
    let default = compute;
    next_non_specialized_queue().unwrap_or(default)
  };

  SingleQueues {
    #[cfg(feature = "graphics_family")]
    graphics,
    #[cfg(feature = "compute_family")]
    compute,
    #[cfg(feature = "transfer_family")]
    transfer,
  }
}

#[cfg(feature = "graphics_family")]
impl QueueFamilies {
  pub const FAMILY_COUNT: usize =
    1 + cfg!(feature = "compute_family") as usize + cfg!(feature = "transfer_family") as usize;

  pub fn get_from_physical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
  ) -> Result<Self, ()> {
    let properties =
      unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let mut graphics = None;
    #[cfg(feature = "compute_family")]
    let mut compute = None; // non graphics
    #[cfg(feature = "transfer_family")]
    let mut transfer = None; // non graphics and non compute
    for (i, props) in properties.into_iter().enumerate() {
      let family = Some(QueueFamily {
        index: i as u32,
        queue_count: props.queue_count,
      });
      if props.queue_flags.contains(vk::QueueFlags::GRAPHICS) && graphics.is_none() {
        graphics = family;
        continue;
      }
      #[cfg(feature = "compute_family")]
      if props.queue_flags.contains(vk::QueueFlags::COMPUTE) && compute.is_none() {
        compute = family;
        continue;
      }
      #[cfg(feature = "transfer_family")]
      if props.queue_flags.contains(vk::QueueFlags::TRANSFER) && transfer.is_none() {
        transfer = family;
        continue;
      }
    }

    if graphics.is_none() {
      return Err(());
    }
    Ok(QueueFamilies {
      graphics: graphics.unwrap(),
      #[cfg(feature = "compute_family")]
      compute,
      #[cfg(feature = "transfer_family")]
      transfer,
    })
  }
}

#[cfg(not(feature = "graphics_family"))]
impl QueueFamilies {
  pub const FAMILY_COUNT: usize = 1 + cfg!(feature = "transfer_family") as usize;

  pub fn get_from_physical_device(
    instance: &ash::Instance,
    physical_device: vk::PhysicalDevice,
  ) -> Result<Self, ()> {
    let properties =
      unsafe { instance.get_physical_device_queue_family_properties(physical_device) };

    let mut compute = None; // non graphics
    #[cfg(feature = "transfer_family")]
    let mut transfer = None; // non graphics and non compute
    for (i, props) in properties.into_iter().enumerate() {
      let family = Some(QueueFamily {
        index: i as u32,
        queue_count: props.queue_count,
      });
      if props.queue_flags.contains(vk::QueueFlags::COMPUTE) && compute.is_none() {
        compute = family;
        continue;
      }
      #[cfg(feature = "transfer_family")]
      if props.queue_flags.contains(vk::QueueFlags::TRANSFER) && transfer.is_none() {
        transfer = family;
        continue;
      }
    }

    if compute.is_none() {
      return Err(());
    }
    Ok(QueueFamilies {
      compute: compute.unwrap(),
      #[cfg(feature = "transfer_family")]
      transfer,
    })
  }
}
