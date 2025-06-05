use hotline::{HotlineObject, LibraryRegistry, ObjectHandle};
use std::sync::{Arc, Mutex};

pub const RUSTC_COMMIT: &str = env!("RUSTC_COMMIT_HASH");

pub struct DirectRuntime {
    library_registry: LibraryRegistry,
}

impl DirectRuntime {
    pub fn new() -> Self {
        Self { library_registry: LibraryRegistry::new() }
    }

    #[cfg(target_os = "macos")]
    pub fn new_with_custom_loader() -> Self {
        Self { library_registry: LibraryRegistry::new_with_custom_loader() }
    }

    pub fn register(&mut self, obj: Box<dyn HotlineObject>) -> ObjectHandle {
        Arc::new(Mutex::new(obj))
    }

    pub fn hot_reload(&mut self, lib_path: &str, _type_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.library_registry.load(lib_path)?;
        Ok(())
    }

    // Create object from loaded library
    pub fn create_from_lib(
        &mut self,
        lib_name: &str,
        type_name: &str,
    ) -> Result<ObjectHandle, Box<dyn std::error::Error>> {
        // Get a pointer to self that we can use as 'static
        // This is safe because we know the runtime is leaked in main.rs
        let self_ptr = self as *const DirectRuntime;
        let lib_registry = unsafe { &(*self_ptr).library_registry as &'static LibraryRegistry };

        // Set the library registry in thread-local storage before creating objects
        // This allows constructors to create other objects
        hotline::set_library_registry(lib_registry);

        // Create the object
        let mut obj = lib_registry.call_constructor(lib_name, type_name, RUSTC_COMMIT)?;

        // Store the registry on the object so it can create other objects later
        obj.set_registry(lib_registry);

        let handle = self.register(obj);
        Ok(handle)
    }

    pub fn library_registry(&self) -> &LibraryRegistry {
        &self.library_registry
    }
}