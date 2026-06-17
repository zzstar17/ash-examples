use std::mem::MaybeUninit;

use vkobjects::DeviceManuallyDestroyed;

pub struct Destructor<const N: usize> {
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
