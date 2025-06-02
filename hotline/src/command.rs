use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::any::Any;
use crate::HotlineObject;

/// Registry for loaded libraries that can be shared between runtime and hotline
#[derive(Clone)]
pub struct LibraryRegistry {
    libs: Arc<Mutex<HashMap<String, Arc<Library>>>>,
}

impl LibraryRegistry {
    pub fn new() -> Self {
        Self {
            libs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    
    pub fn load(&self, lib_path: &str) -> Result<String, Box<dyn std::error::Error>> {
        let lib = unsafe { Library::new(lib_path)? };
        let lib_name = std::path::Path::new(lib_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid lib path")?
            .to_string();
        
        println!("LibraryRegistry::load - loading library from path: {} with name: {}", lib_path, lib_name);
        let mut libs = self.libs.lock().unwrap();
        libs.insert(lib_name.clone(), Arc::new(lib));
        println!("LibraryRegistry::load - library loaded successfully");
        Ok(lib_name)
    }
    
    pub fn with_symbol<T, R, F>(&self, lib_name: &str, symbol_name: &str, f: F) -> Result<R, Box<dyn std::error::Error>> 
    where
        T: 'static,
        F: FnOnce(&Symbol<T>) -> R,
    {
        println!("LibraryRegistry::with_symbol - attempting to lock libs mutex...");
        
        // Get Arc<Library> while holding the lock, then release the lock
        let lib_arc = {
            let libs = self.libs.lock().unwrap();
            println!("LibraryRegistry::with_symbol - mutex locked successfully");
            println!("LibraryRegistry::with_symbol - mutex locked, looking for library: {}", lib_name);
            println!("LibraryRegistry::with_symbol - available libraries: {:?}", libs.keys().collect::<Vec<_>>());
            libs.get(lib_name).ok_or_else(|| {
                format!("library '{}' not loaded. Available: {:?}", lib_name, libs.keys().collect::<Vec<_>>())
            })?.clone()
        }; // mutex is dropped here
        
        println!("LibraryRegistry::with_symbol - mutex released, looking for symbol: {}", symbol_name);
        let symbol: Symbol<T> = unsafe { lib_arc.get(symbol_name.as_bytes())? };
        println!("LibraryRegistry::with_symbol - found symbol, calling function");
        
        Ok(f(&symbol))
    }
    
    
    pub fn call_constructor(&self, lib_name: &str, type_name: &str, rustc_commit: &str) -> Result<Box<dyn HotlineObject>, Box<dyn std::error::Error>> {
        let constructor_symbol = format!("{}__new____to__Box_lt_dyn_HotlineObject_gt__{}", type_name, rustc_commit);
        println!("LibraryRegistry::call_constructor - looking for symbol: {} in library: {}", constructor_symbol, lib_name);
        type ConstructorFn = fn() -> Box<dyn HotlineObject>;
        
        self.with_symbol::<ConstructorFn, _, _>(lib_name, &constructor_symbol, |symbol| {
            println!("LibraryRegistry::call_constructor - found symbol, calling constructor...");
            let result = symbol();
            println!("LibraryRegistry::call_constructor - constructor returned");
            result
        })
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
    rustc_commit: String,
}

impl CommandRegistry {
    pub fn new(library_registry: LibraryRegistry, rustc_commit: String) -> Self {
        Self {
            library_registry,
            rustc_commit,
        }
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