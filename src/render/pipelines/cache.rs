use core::slice;
use std::{
  fs::{self, File},
  hash::{DefaultHasher, Hash, Hasher},
  io::{self, Read, Write},
  mem::{self, size_of},
  os::raw::c_void,
  ptr::addr_of,
};

use ash::vk;

use crate::render::{errors::OutOfMemoryError, initialization::device::PhysicalDevice};

// random number used to identify that the file type is correct
// this is not that reliable but its better than not having it
const MAGIC: u32 = 0x74c1887f;

const TEMP_PATH: &str = "./pipeline_cache.temp";
const PATH: &str = "pipeline_cache";

fn hash_data(data: &Vec<u8>) -> u64 {
  let mut hasher = DefaultHasher::new();
  data.hash(&mut hasher);
  hasher.finish()
}

// https://medium.com/@zeuxcg/creating-a-robust-pipeline-cache-with-vulkan-961d09416cda
#[derive(Debug, PartialEq, Eq)]
#[repr(C)]
struct PipelineCacheHeader {
  magic: u32,
  vendor_id: u32,
  device_id: u32,
  driver_version: u32,
  driver_abi: u32,

  data_size: u32,
  data_hash: u64,
  cache_uuid: [u8; vk::UUID_SIZE],
}

#[derive(thiserror::Error, Debug)]
pub enum PipelineCacheError {
  #[error(transparent)]
  IOError(#[from] io::Error),
  #[error(transparent)]
  OutOfMemoryError(#[from] OutOfMemoryError),
}

impl PipelineCacheHeader {
  pub fn generate(physical_device: &PhysicalDevice, data: &Vec<u8>) -> Self {
    let props = &physical_device.properties;
    Self {
      magic: MAGIC,
      vendor_id: props.vendor_id,
      device_id: props.device_id,
      driver_version: props.driver_version,
      driver_abi: size_of::<*const c_void>() as u32,
      cache_uuid: props.pipeline_cache_uuid,
      data_size: data.len() as u32,
      data_hash: hash_data(data),
    }
  }

  fn is_compatible(&self, physical_device: &PhysicalDevice) -> bool {
    let props = &physical_device.properties;
    self.magic == MAGIC
      && self.vendor_id == props.vendor_id
      && self.device_id == props.device_id
      && self.driver_version == props.driver_version
      && self.driver_abi == size_of::<*const c_void>() as u32
      && self.cache_uuid == props.pipeline_cache_uuid
  }

  fn bytes<'a>(&self) -> &'a [u8] {
    unsafe { slice::from_raw_parts(addr_of!(*self) as *const u8, size_of::<Self>()) }
  }

  unsafe fn from_bytes(bytes: [u8; size_of::<Self>()]) -> PipelineCacheHeader {
    mem::transmute(bytes)
  }
}

// tries to save the pipeline cache data to a file
pub fn save_pipeline_cache(
  device: &ash::Device,
  physical_device: &PhysicalDevice,
  pipeline_cache: vk::PipelineCache,
) -> Result<(), PipelineCacheError> {
  let data = unsafe {
    device
      .get_pipeline_cache_data(pipeline_cache)
      .map_err(|err| PipelineCacheError::OutOfMemoryError(err.into()))?
  };
  let header = PipelineCacheHeader::generate(physical_device, &data);

  {
    let mut temp = File::create(TEMP_PATH)?;
    temp.write_all(header.bytes())?;
    temp.write_all(data.as_slice())?;
    temp.sync_data()?;
  }

  fs::copy(TEMP_PATH, PATH)?;
  fs::remove_file(TEMP_PATH)?;

  Ok(())
}

pub fn create_pipeline_cache(
  device: &ash::Device,
  physical_device: &PhysicalDevice,
) -> Result<(vk::PipelineCache, bool), PipelineCacheError> {
  // tries to create a pipeline cache from an existing file
  let cache_result = match try_read_pipeline_cache_data_from_file(physical_device) {
    Ok(data) => {
      let create_info = vk::PipelineCacheCreateInfo::default().initial_data(&data);
      let result = unsafe { device.create_pipeline_cache(&create_info, None) };

      result.map_err(|err| {
        log::error!(
          "Pipeline cache file data was retrieved however pipeline creation operation failed: {:?}",
          err
        );
      })
    }
    Err(err) => {
      // it's okay if file doesn't exist
      if err.kind() != io::ErrorKind::NotFound {
        log::error!(
          "Pipeline cache file exists however it is incompatible or corrupted: {:?}",
          err
        );
      }

      Err(())
    }
  };

  match cache_result {
    Ok(cache) => Ok((cache, true)),
    Err(()) => {
      let create_info = vk::PipelineCacheCreateInfo::default();
      let cache = unsafe {
        device
          .create_pipeline_cache(&create_info, None)
          .map_err(|err| PipelineCacheError::OutOfMemoryError(err.into()))?
      };
      Ok((cache, false))
    }
  }
}

fn try_read_pipeline_cache_data_from_file(physical_device: &PhysicalDevice) -> io::Result<Vec<u8>> {
  let mut file = File::open(PATH)?;

  let mut header_bytes = [0u8; size_of::<PipelineCacheHeader>()];
  file.read_exact(&mut header_bytes)?;

  let header = unsafe { PipelineCacheHeader::from_bytes(header_bytes) };
  if !header.is_compatible(physical_device) {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "Header file is incompatible or corrupted",
    ));
  }

  let mut data = Vec::new();
  file.read_to_end(&mut data)?;
  if data.len() != header.data_size as usize || hash_data(&data) != header.data_hash {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      "Pipeline cache data is corrupted",
    ));
  }

  Ok(data)
}
