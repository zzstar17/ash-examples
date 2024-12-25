macro_rules! destroy {
  ($($obj:expr),+) => {
    {
      $(ash_destructor::SelfDestroyable::destroy_self($obj);)+
    }
  };

  ($device:expr => $($obj:expr),+) => {
    {
      $(ash_destructor::DeviceDestroyable::destroy_self($obj, $device);)+
    }
  };
}
pub(crate) use destroy;
