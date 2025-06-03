use crate::HotlineObject;
use libloading::{Library, Symbol};
use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Registry for loaded libraries that can be shared between runtime and hotline
#[derive(Clone)]
pub struct LibraryRegistry {
    libs: Arc<Mutex<HashMap<String, Arc<Library>>>>,
}

impl LibraryRegistry {
    pub fn new() -> Self {
        Self { libs: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn load(&self, lib_path: &str) -> Result<String, Box<dyn std::error::Error>> {
        let lib = unsafe { Library::new(lib_path)? };
        let lib_name =
            std::path::Path::new(lib_path).file_stem().and_then(|s| s.to_str()).ok_or("invalid lib path")?.to_string();

        let mut libs = self.libs.lock().unwrap();
        libs.insert(lib_name.clone(), Arc::new(lib));
        Ok(lib_name)
    }

    pub fn with_symbol<T, R, F>(&self, lib_name: &str, symbol_name: &str, f: F) -> Result<R, Box<dyn std::error::Error>>
    where
        T: 'static,
        F: FnOnce(&Symbol<T>) -> R,
    {
        // Get Arc<Library> while holding the lock, then release the lock
        let lib_arc = {
            let libs = self.libs.lock().unwrap();
            libs.get(lib_name)
                .ok_or_else(|| {
                    format!("library '{}' not loaded. Available: {:?}", lib_name, libs.keys().collect::<Vec<_>>())
                })?
                .clone()
        }; // mutex is dropped here

        let symbol: Symbol<T> = unsafe { lib_arc.get(symbol_name.as_bytes())? };

        Ok(f(&symbol))
    }

    pub fn call_constructor(
        &self,
        lib_name: &str,
        type_name: &str,
        rustc_commit: &str,
    ) -> Result<Box<dyn HotlineObject>, Box<dyn std::error::Error>> {
        let constructor_symbol = format!("{}__new____to__Box_lt_dyn_HotlineObject_gt__{}", type_name, rustc_commit);
        type ConstructorFn = fn() -> Box<dyn HotlineObject>;

        self.with_symbol::<ConstructorFn, _, _>(lib_name, &constructor_symbol, |symbol| symbol())
    }
}

/// Command interface for inter-object method calls
pub trait CommandHandler {
    fn call_method(
        &self,
        obj: &mut dyn Any,
        lib_name: &str,
        type_name: &str,
        method_name: &str,
        args: Vec<Box<dyn Any>>,
    ) -> Result<Box<dyn Any>, Box<dyn std::error::Error>>;
}

pub struct CommandRegistry {
    library_registry: LibraryRegistry,
}

impl CommandRegistry {
    pub fn new(library_registry: LibraryRegistry) -> Self {
        Self { library_registry }
    }

    pub fn library_registry(&self) -> &LibraryRegistry {
        &self.library_registry
    }
}

impl CommandHandler for CommandRegistry {
    fn call_method(
        &self,
        _obj: &mut dyn Any,
        _lib_name: &str,
        _type_name: &str,
        _method_name: &str,
        _args: Vec<Box<dyn Any>>,
    ) -> Result<Box<dyn Any>, Box<dyn std::error::Error>> {
        // This would generate the correct symbol name and call it
        // For now, just a placeholder
        Err("Not implemented".into())
    }
}
