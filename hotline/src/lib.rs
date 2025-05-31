pub mod typed;
pub use typed::*;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub u64);