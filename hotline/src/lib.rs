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
