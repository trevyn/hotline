pub use hotline_macros::object;

use std::any::Any;
use std::sync::{Arc, Mutex};

// Re-export libloading for objects to use
pub use libloading;

// Rustc commit hash for symbol generation
pub const RUSTC_COMMIT: &str = env!("RUSTC_COMMIT_HASH");

pub mod command;
pub use command::{CommandHandler, CommandRegistry, LibraryRegistry};

/// Macro to safely call a symbol from a library
/// The Symbol must be kept alive until after the function call
#[macro_export]
macro_rules! call_symbol {
    ($registry:expr, $lib_name:expr, $symbol_name:expr, $fn_type:ty, |$sym:ident| $body:expr) => {{
        match $registry.get_symbol::<$fn_type>($lib_name, $symbol_name) {
            Ok($sym) => {
                let result = $body;
                Ok(result)
            }
            Err(e) => Err(e),
        }
    }};
}

// Removed thread_local approach - objects now get LibraryRegistry via init function

pub trait HotlineObject: Any + Send + Sync {
    fn type_name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

pub type ObjectHandle = Arc<Mutex<Box<dyn HotlineObject>>>;

/// wrapper for duck-typed objects that can act like T
pub struct Like<T> {
    handle: ObjectHandle,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Like<T> {
    pub fn new(handle: ObjectHandle) -> Self {
        Self {
            handle,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn handle(&self) -> &ObjectHandle {
        &self.handle
    }
}

// Global registry access for typed objects
thread_local! {
    static LIBRARY_REGISTRY: std::cell::RefCell<Option<&'static LibraryRegistry>> = std::cell::RefCell::new(None);
}

pub fn set_library_registry(registry: &'static LibraryRegistry) {
    LIBRARY_REGISTRY.with(|r| {
        *r.borrow_mut() = Some(registry);
    });
}

pub fn with_library_registry<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&LibraryRegistry) -> R,
{
    LIBRARY_REGISTRY.with(|r| r.borrow().map(f))
}

use std::marker::PhantomData;

/// Typed wrapper for hotline objects that provides clean method dispatch
pub struct ObjectRef<T> {
    inner: ObjectHandle,
    _phantom: PhantomData<T>,
}

impl<T> ObjectRef<T> {
    pub fn new(inner: ObjectHandle) -> Self {
        Self {
            inner,
            _phantom: PhantomData,
        }
    }
    
    pub fn inner(&self) -> &ObjectHandle {
        &self.inner
    }
}

impl<T> Clone for ObjectRef<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _phantom: PhantomData,
        }
    }
}
