pub use hotline_macros::object;

use std::any::Any;
use std::sync::{Arc, Mutex, OnceLock};

// Re-export libloading for objects to use
pub use libloading;

// Re-export tokio for async support
pub use tokio;

// Global tokio runtime for all hotline objects
static HOTLINE_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

pub fn hotline_runtime() -> &'static tokio::runtime::Runtime {
    HOTLINE_RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .enable_all()
            .build()
            .expect("Failed to create hotline runtime")
    })
}

// Rustc commit hash for symbol generation
pub const RUSTC_COMMIT: &str = env!("RUSTC_COMMIT_HASH");

pub mod command;
#[cfg(target_os = "macos")]
mod macho_loader;
#[cfg(not(target_os = "macos"))]
mod macho_loader {}

#[cfg(target_os = "macos")]
mod tlv_support;
#[cfg(not(target_os = "macos"))]
mod tlv_support {}

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

// Thread-local storage for the current library registry
// Now that TLV support is working, we can use this for object creation
thread_local! {
    static CURRENT_REGISTRY: std::cell::RefCell<Option<&'static LibraryRegistry>> = std::cell::RefCell::new(None);
}

// Set the current thread's library registry
pub fn set_library_registry(registry: &'static LibraryRegistry) {
    CURRENT_REGISTRY.with(|r| {
        // Try to borrow. If we can't (because we're already inside with_library_registry),
        // just skip - we're already in the right context
        if let Ok(mut borrowed) = r.try_borrow_mut() {
            *borrowed = Some(registry);
        }
        // If we can't borrow, we're already inside a with_library_registry call,
        // so the registry is already available
    });
}

// Access the current thread's library registry
pub fn with_library_registry<T, F>(f: F) -> Option<T>
where
    F: FnOnce(&'static LibraryRegistry) -> T,
{
    CURRENT_REGISTRY.with(|r| r.borrow().as_ref().map(|registry| f(registry)))
}

pub trait HotlineObject: Any + Send + Sync {
    fn type_name(&self) -> &'static str;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn set_registry(&mut self, registry: &'static LibraryRegistry);
    fn get_registry(&self) -> Option<&'static LibraryRegistry>;
}

pub type ObjectHandle = Arc<Mutex<Box<dyn HotlineObject>>>;

/// wrapper for duck-typed objects that can act like T
pub struct Like<T> {
    handle: ObjectHandle,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> Like<T> {
    pub fn new(handle: ObjectHandle) -> Self {
        Self { handle, _phantom: std::marker::PhantomData }
    }

    pub fn handle(&self) -> &ObjectHandle {
        &self.handle
    }
}

// Objects now store their own registry reference - no thread_local needed

// Safe wrapper for registry pointer that implements Send + Sync
#[doc(hidden)]
#[derive(Clone)]
pub struct RegistryPtr(Option<std::ptr::NonNull<LibraryRegistry>>);

unsafe impl Send for RegistryPtr {}
unsafe impl Sync for RegistryPtr {}

impl RegistryPtr {
    pub fn new() -> Self {
        Self(None)
    }

    pub fn set(&mut self, registry: &'static LibraryRegistry) {
        self.0 = std::ptr::NonNull::new(registry as *const _ as *mut _);
    }

    pub fn get(&self) -> Option<&'static LibraryRegistry> {
        self.0.map(|ptr| unsafe { &*ptr.as_ptr() })
    }
}

impl Default for RegistryPtr {
    fn default() -> Self {
        Self::new()
    }
}

use std::marker::PhantomData;

/// Typed wrapper for hotline objects that provides clean method dispatch
pub struct ObjectRef<T> {
    inner: ObjectHandle,
    _phantom: PhantomData<T>,
}

impl<T> ObjectRef<T> {
    pub fn new(inner: ObjectHandle) -> Self {
        Self { inner, _phantom: PhantomData }
    }

    pub fn inner(&self) -> &ObjectHandle {
        &self.inner
    }
}

impl<T> Clone for ObjectRef<T> {
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone(), _phantom: PhantomData }
    }
}

// Event handling trait for objects
pub trait EventHandler: Send + Sync {
    fn handle_mouse_down(&mut self, _x: f64, _y: f64) -> bool {
        false
    }
    fn handle_mouse_up(&mut self, _x: f64, _y: f64) -> bool {
        false
    }
    fn handle_mouse_move(&mut self, _x: f64, _y: f64) -> bool {
        false
    }
    fn handle_mouse_wheel(&mut self, _x: f64, _y: f64, _delta: f64) -> bool {
        false
    }
    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
    fn handle_key_down(&mut self, _keycode: i32, _shift: bool) -> bool {
        false
    } // keycode as i32 to avoid sdl3 dependency
    fn is_focused(&self) -> bool {
        false
    }
    fn update(&mut self) {}
    fn render(&mut self, _buffer: &mut [u8], _width: i64, _height: i64, _pitch: i64) {}
}
