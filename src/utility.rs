use std::ffi::{CStr, FromBytesUntilNulError};

use ash::vk;

// this module contains general functions used in other modules

pub fn parse_vulkan_api_version(v: u32) -> String {
  format!(
    "{}.{}.{}",
    vk::api_version_major(v),
    vk::api_version_minor(v),
    vk::api_version_patch(v)
  )
}

pub unsafe fn i8_array_as_cstr(arr: &[i8]) -> Result<&CStr, FromBytesUntilNulError> {
  CStr::from_bytes_until_nul(std::mem::transmute::<&[i8], &[u8]>(arr))
}

// power is the actual power of 2 (1, 2, 4, 8, 16...)
#[inline]
#[allow(dead_code)]
pub fn round_down_to_power_of_2_u64(n: u64, power: u64) -> u64 {
  // strip right-most <less than power> bits from n
  let rem_error = n & (power - 1);
  n - rem_error
}

// power is the actual power of 2 (1, 2, 4, 8, 16...)
// basically same as rounded_down_to_power_of_2 but
//    adds power to rounded_down_to_power_of_2(n) if n does not already divide power
#[inline]
pub fn round_up_to_power_of_2_u64(n: u64, power: u64) -> u64 {
  let rem_error = n & (power - 1);
  if rem_error > 0 {
    n - rem_error + power
  } else {
    n
  }
}

pub trait OnErr<T, E> {
  fn on_err<O: FnOnce(&E)>(self, op: O) -> Result<T, E>
  where
    Self: Sized;
}

impl<T, E> OnErr<T, E> for Result<T, E> {
  fn on_err<O: FnOnce(&E)>(self, op: O) -> Result<T, E>
  where
    Self: Sized,
  {
    if let Err(ref e) = self {
      op(e);
    }
    self
  }
}

macro_rules! const_flag_bitor {
  ($t:ty, $x:expr, $($y:expr),+) => {
    // ash flags don't implement const bitor
    <$t>::from_raw(
      $x.as_raw() $(| $y.as_raw())+,
    )
  };
}
pub(crate) use const_flag_bitor;
