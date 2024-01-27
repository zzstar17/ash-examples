use std::{
  ffi::c_void,
  mem::MaybeUninit,
  ops::BitOr,
  ptr::{self, addr_of_mut},
};

use ash::vk;

use crate::{
  utility::{self, c_char_array_to_string},
  IMAGE_FORMAT, IMAGE_HEIGHT, IMAGE_WIDTH, REQUIRED_DEVICE_EXTENSIONS, TARGET_API_VERSION,
};

macro_rules! const_flag_bitor {
  ($t:ty, $x:expr, $($y:expr),+) => {
    // ash flags don't implement const bitor
    <$t>::from_raw(
      $x.as_raw() $(| $y.as_raw())+,
    )
  };
}

// kinda overkill
const REQUIRED_FORMAT_IMAGE_FLAGS_OPTIMAL: vk::FormatFeatureFlags = const_flag_bitor!(
  vk::FormatFeatureFlags,
  vk::FormatFeatureFlags::TRANSFER_SRC,
  vk::FormatFeatureFlags::TRANSFER_DST
);
const REQUIRED_FORMAT_IMAGE_FLAGS_LINEAR: vk::FormatFeatureFlags =
  vk::FormatFeatureFlags::TRANSFER_DST;

const REQUIRED_IMAGE_USAGE_FLAGS_OPTIMAL: vk::ImageUsageFlags = const_flag_bitor!(
  vk::ImageUsageFlags,
  vk::ImageUsageFlags::TRANSFER_SRC,
  vk::ImageUsageFlags::TRANSFER_DST
);
const REQUIRED_IMAGE_USAGE_FLAGS_LINEAR: vk::ImageUsageFlags = vk::ImageUsageFlags::TRANSFER_DST;

#[derive(Debug)]
pub struct QueueFamily {
  pub index: u32,
  pub queue_count: u32,
}

// Specialized compute and transfer queue families may not be available
// If so, they will be substituted by the graphics queue family, as a queue family that supports
//    graphics implicitly also supports compute and transfer operations
#[derive(Debug)]
pub struct QueueFamilies {
  pub graphics: QueueFamily,
  pub compute: Option<QueueFamily>,
  pub transfer: Option<QueueFamily>,
  pub unique_indices: Box<[u32]>,
}

impl QueueFamilies {
  pub fn get_compute_index(&self) -> u32 {
    match self.compute.as_ref() {
      Some(family) => family.index,
      None => self.graphics.index,
    }
  }

  pub fn get_transfer_index(&self) -> u32 {
    match self.transfer.as_ref() {
      Some(family) => family.index,
      None => self.graphics.index,
    }
  }
}

enum Vendor {
  NVIDIA,
  AMD,
  ARM,
  INTEL,
  ImgTec,
  Qualcomm,
  Unknown(u32),
}

// support struct for displaying vendor information
impl Vendor {
  fn from_id(id: u32) -> Self {
    // some known ids
    match id {
      0x1002 => Self::AMD,
      0x1010 => Self::ImgTec,
      0x10DE => Self::NVIDIA,
      0x13B5 => Self::ARM,
      0x5143 => Self::Qualcomm,
      0x8086 => Self::INTEL,
      _ => Self::Unknown(id),
    }
  }

  fn parse_driver_version(&self, v: u32) -> String {
    // Different vendors can use their own version formats
    // The Vulkan format is (3 bits), major (7 bits), minor (10 bits), patch (12 bits), so vendors
    // with other formats need their own parsing code
    match self {
      Self::NVIDIA => {
        // major (10 bits), minor (8 bits), secondary branch (8 bits), tertiary branch (6 bits)
        let eight_bits = 0b11111111;
        let six_bits = 0b111111;
        format!(
          "{}.{}.{}.{}",
          v >> (32 - 10),
          v >> (32 - 10 - 8) & eight_bits,
          v >> (32 - 10 - 8 - 8) & eight_bits,
          v & six_bits
        )
      }
      _ => utility::parse_vulkan_api_version(v),
    }
  }
}

impl ToString for Vendor {
  fn to_string(&self) -> String {
    match self {
      Self::NVIDIA => "NVIDIA".to_owned(),
      Self::AMD => "AMD".to_owned(),
      Self::ARM => "ARM".to_owned(),
      Self::INTEL => "INTEL".to_owned(),
      Self::ImgTec => "ImgTec".to_owned(),
      Self::Qualcomm => "Qualcomm".to_owned(),
      Self::Unknown(id) => format!("Unknown ({})", id),
    }
  }
}

fn log_device_properties(properties: &vk::PhysicalDeviceProperties) {
  let vendor = Vendor::from_id(properties.vendor_id);
  let driver_version = vendor.parse_driver_version(properties.driver_version);

  log::info!(
    "\nFound physical device \"{}\":
    API Version: {},
    Vendor: {},
    Driver Version: {},
    ID: {},
    Type: {},",
    c_char_array_to_string(&properties.device_name),
    utility::parse_vulkan_api_version(properties.api_version),
    vendor.to_string(),
    driver_version,
    properties.device_id,
    match properties.device_type {
      vk::PhysicalDeviceType::INTEGRATED_GPU => "Integrated GPU",
      vk::PhysicalDeviceType::DISCRETE_GPU => "Discrete GPU",
      vk::PhysicalDeviceType::VIRTUAL_GPU => "Virtual GPU",
      vk::PhysicalDeviceType::CPU => "CPU",
      _ => "Unknown",
    },
  );
}

fn check_extension_support(instance: &ash::Instance, device: vk::PhysicalDevice) -> bool {
  let properties = unsafe {
    instance
      .enumerate_device_extension_properties(device)
      .expect("Failed to get device extension properties")
  };

  let mut available: Vec<String> = properties
    .into_iter()
    .map(|prop| utility::c_char_array_to_string(&prop.extension_name))
    .collect();

  utility::not_in_slice(
    available.as_mut_slice(),
    &mut REQUIRED_DEVICE_EXTENSIONS.iter(),
    |av, req| av.as_str().cmp(req.to_str().unwrap()),
  )
  .is_empty()
}

fn check_format_support(instance: &ash::Instance, physical_device: vk::PhysicalDevice) -> bool {
  let properties =
    unsafe { instance.get_physical_device_format_properties(physical_device, IMAGE_FORMAT) };

  if !properties
    .optimal_tiling_features
    .contains(REQUIRED_FORMAT_IMAGE_FLAGS_OPTIMAL)
  {
    return false;
  }

  if !properties
    .linear_tiling_features
    .contains(REQUIRED_FORMAT_IMAGE_FLAGS_LINEAR)
  {
    return false;
  }

  true
}

fn check_image_size_support(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
  tiling: vk::ImageTiling,
  usage: vk::ImageUsageFlags,
) -> bool {
  let properties = unsafe {
    instance
      .get_physical_device_image_format_properties(
        physical_device,
        IMAGE_FORMAT,
        vk::ImageType::TYPE_2D,
        tiling,
        usage,
        vk::ImageCreateFlags::empty(),
      )
      .expect("Failed to query for image format properties")
  };
  log::debug!(
    "{} image {:?} properties: {:#?}",
    match tiling {
      vk::ImageTiling::LINEAR => "Linear",
      vk::ImageTiling::OPTIMAL => "Optimal",
      _ => panic!(),
    },
    IMAGE_FORMAT,
    properties
  );

  IMAGE_WIDTH <= properties.max_extent.width && IMAGE_HEIGHT <= properties.max_extent.height
}

fn check_linear_tiling_image_size_support(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> bool {
  check_image_size_support(
    instance,
    physical_device,
    vk::ImageTiling::LINEAR,
    REQUIRED_IMAGE_USAGE_FLAGS_LINEAR,
  )
}

fn check_optimal_tiling_image_size_support(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> bool {
  check_image_size_support(
    instance,
    physical_device,
    vk::ImageTiling::OPTIMAL,
    REQUIRED_IMAGE_USAGE_FLAGS_OPTIMAL,
  )
}

unsafe fn select_physical_device(
  instance: &ash::Instance,
) -> Option<(vk::PhysicalDevice, QueueFamilies)> {
  instance
    .enumerate_physical_devices()
    .expect("Failed to enumerate physical devices")
    .into_iter()
    .filter(|&physical_device| {
      // Filter devices that are not supported
      // You should check for any feature or limit support that your application might need

      let properties = instance.get_physical_device_properties(physical_device);
      log_device_properties(&properties);

      if properties.api_version < TARGET_API_VERSION {
        log::info!(
          "Skipped physical device: Device API version is less than targeted by the application"
        );
        return false;
      }

      // check if device supports all required extensions
      if !check_extension_support(instance, physical_device) {
        log::info!("Skipped physical device: Device does not support all required extensions");
        return false;
      }

      if !check_format_support(instance, physical_device) {
        log::warn!("Skipped physical device: Device does not support required formats");
        return false;
      }

      if !check_linear_tiling_image_size_support(instance, physical_device) || !check_optimal_tiling_image_size_support(instance, physical_device) {
        log::warn!("Skipped physical device: Application image size requirements are bigger than supported by the device");
        return false;
      }

      true
    })
    .filter_map(|physical_device| {
      // Filter devices that not support specific queue families
      // Your application may not need any graphics capabilities or otherwise need features only
      //    supported by specific queues, so alter to your case accordingly
      // Generally you only need one queue from each family unless you are doing highly concurrent
      //    operations

      let mut graphics = None;
      let mut compute = None;
      let mut transfer = None;
      for (i, family) in instance
        .get_physical_device_queue_family_properties(physical_device)
        .iter()
        .enumerate()
      {
        if family.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
          if graphics.is_none() {
            graphics = Some(QueueFamily {
              index: i as u32,
              queue_count: family.queue_count,
            });
          }
        } else if family.queue_flags.contains(vk::QueueFlags::COMPUTE) {
          // only set if family does not contain graphics flag
          if compute.is_none() {
            compute = Some(QueueFamily {
              index: i as u32,
              queue_count: family.queue_count,
            });
          }
        } else if family.queue_flags.contains(vk::QueueFlags::TRANSFER) {
          // only set if family does not contain graphics nor compute flag
          if transfer.is_none() {
            transfer = Some(QueueFamily {
              index: i as u32,
              queue_count: family.queue_count,
            });
          }
        }
      }

      if graphics.is_none() {
        log::info!("Skipped physical device: Device does not support graphics");
        return None;
      }

      // commonly used
      let unique_indices = [graphics.as_ref(), compute.as_ref(), transfer.as_ref()]
        .into_iter()
        .filter_map(|opt| opt.map(|f| f.index))
        .collect();

      Some((
        physical_device,
        QueueFamilies {
          graphics: graphics.unwrap(),
          compute,
          transfer,
          unique_indices,
        },
      ))
    })
    .min_by_key(|(physical_device, families)| {
      // Assign a score to each device and select the best one available
      // A full application may use multiple metrics like limits, queue families and even the
      //    device id to rank each device that a user can have

      let queue_family_importance = 3;
      let device_score_importance = 0;

      // rank devices by number of specialized queue families
      let queue_score = 3 - families.unique_indices.len();

      // rank devices by commonly most powerful device type
      let device_score = match instance
        .get_physical_device_properties(*physical_device)
        .device_type
      {
        vk::PhysicalDeviceType::DISCRETE_GPU => 0,
        vk::PhysicalDeviceType::INTEGRATED_GPU => 1,
        vk::PhysicalDeviceType::VIRTUAL_GPU => 2,
        vk::PhysicalDeviceType::CPU => 3,
        vk::PhysicalDeviceType::OTHER => 4,
        _ => 5,
      };

      (queue_score << queue_family_importance) + (device_score << device_score_importance)
    })
}

fn get_extended_properties(
  instance: &ash::Instance,
  physical_device: vk::PhysicalDevice,
) -> (
  vk::PhysicalDeviceProperties,
  vk::PhysicalDeviceVulkan11Properties,
) {
  // going c style (see https://doc.rust-lang.org/std/mem/union.MaybeUninit.html)
  let mut main_props: MaybeUninit<vk::PhysicalDeviceProperties2> = MaybeUninit::uninit();
  let mut props11: MaybeUninit<vk::PhysicalDeviceVulkan11Properties> = MaybeUninit::uninit();
  let main_props_ptr = main_props.as_mut_ptr();
  let props11_ptr = props11.as_mut_ptr();

  unsafe {
    addr_of_mut!((*props11_ptr).s_type)
      .write(vk::StructureType::PHYSICAL_DEVICE_VULKAN_1_1_PROPERTIES);
    addr_of_mut!((*props11_ptr).p_next).write(ptr::null_mut::<c_void>());

    addr_of_mut!((*main_props_ptr).s_type).write(vk::StructureType::PHYSICAL_DEVICE_PROPERTIES_2);
    // requesting for Vulkan11Properties
    addr_of_mut!((*main_props_ptr).p_next).write(props11_ptr as *mut c_void);

    instance.get_physical_device_properties2(physical_device, main_props_ptr.as_mut().unwrap());

    (main_props.assume_init().properties, props11.assume_init())
  }
}

// in order to not query physical device info multiple times, this struct saves the additional information
pub struct PhysicalDevice {
  pub vk_device: vk::PhysicalDevice,
  pub queue_families: QueueFamilies,
  mem_properties: vk::PhysicalDeviceMemoryProperties,
  max_memory_allocation_size: vk::DeviceSize,
}

impl PhysicalDevice {
  pub unsafe fn select(instance: &ash::Instance) -> PhysicalDevice {
    let (physical_device, queue_families) =
      select_physical_device(instance).expect("No supported physical device available");

    let (properties, properties11) = get_extended_properties(instance, physical_device);
    let mem_properties = instance.get_physical_device_memory_properties(physical_device);
    let queue_family_properties =
      instance.get_physical_device_queue_family_properties(physical_device);

    log::info!(
      "Using physical device \"{}\"",
      c_char_array_to_string(&properties.device_name)
    );
    print_queue_families_debug_info(&queue_family_properties);
    print_device_memory_debug_info(&mem_properties);

    PhysicalDevice {
      vk_device: physical_device,
      mem_properties,
      queue_families,
      max_memory_allocation_size: properties11.max_memory_allocation_size,
    }
  }

  pub fn find_memory_type(
    &self,
    required_memory_type_bits: u32,
    required_properties: vk::MemoryPropertyFlags,
  ) -> Result<u32, ()> {
    for (i, memory_type) in self.mem_properties.memory_types.iter().enumerate() {
      let valid_type = required_memory_type_bits & (1 << i) > 0;
      if valid_type && memory_type.property_flags.contains(required_properties) {
        return Ok(i as u32);
      }
    }

    Err(())
  }

  // Tries to find optimal memory type. If it fails, tries to find a memory type with only
  // required flags
  pub fn find_optimal_memory_type(
    &self,
    required_memory_type_bits: u32,
    required_properties: vk::MemoryPropertyFlags,
    optional_properties: vk::MemoryPropertyFlags,
  ) -> Result<u32, ()> {
    self
      .find_memory_type(
        required_memory_type_bits,
        required_properties.bitor(optional_properties),
      )
      .or_else(|()| self.find_memory_type(required_memory_type_bits, required_properties))
  }

  pub fn get_memory_type(&self, type_i: u32) -> vk::MemoryType {
    self.mem_properties.memory_types[type_i as usize]
  }

  pub fn get_memory_type_heap(&self, type_i: u32) -> vk::MemoryHeap {
    let mem_type = self.get_memory_type(type_i);
    self.mem_properties.memory_heaps[mem_type.heap_index as usize]
  }

  pub fn get_max_memory_allocation_size(&self) -> vk::DeviceSize {
    self.max_memory_allocation_size
  }
}

fn print_queue_families_debug_info(properties: &Vec<vk::QueueFamilyProperties>) {
  log::debug!("Queue family properties: {:#?}", properties);
}

fn print_device_memory_debug_info(mem_properties: &vk::PhysicalDeviceMemoryProperties) {
  log::debug!("Available memory heaps:");
  for heap_i in 0..mem_properties.memory_heap_count {
    let heap = mem_properties.memory_heaps[heap_i as usize];
    let heap_flags = if heap.flags.is_empty() {
      String::from("no heap flags")
    } else {
      format!("heap flags [{:?}]", heap.flags)
    };

    log::debug!(
      "    {} -> {}mb with {} and attributed memory types:",
      heap_i,
      heap.size / 1000000,
      heap_flags
    );
    for type_i in 0..mem_properties.memory_type_count {
      let mem_type = mem_properties.memory_types[type_i as usize];
      if mem_type.heap_index != heap_i {
        continue;
      }

      let flags = mem_type.property_flags;
      log::debug!(
        "        {} -> {}",
        type_i,
        if flags.is_empty() {
          "<no flags>".to_owned()
        } else {
          format!("[{:?}]", flags)
        }
      );
    }
  }
}
