use crate::HotlineObject;
#[cfg(target_os = "macos")]
use crate::macho_loader::MachoLoader;
use libloading::{Library, Symbol};
use std::any::Any;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};

enum LoadedLibrary {
    Dlopen(Arc<Library>),
    #[cfg(target_os = "macos")]
    Custom(Arc<Mutex<MachoLoader>>),
}

/// Registry for loaded libraries that can be shared between runtime and hotline
#[derive(Clone)]
pub struct LibraryRegistry {
    libs: Arc<Mutex<HashMap<String, LoadedLibrary>>>,
    use_custom_loader: bool,
    // Keep old libraries mapped to prevent TLV crashes during hot reload
    old_libs: Arc<Mutex<Vec<LoadedLibrary>>>,
    symbol_cache: Arc<Mutex<HashMap<(String, String), *const c_void>>>,
}

impl LibraryRegistry {
    pub fn new() -> Self {
        Self {
            libs: Arc::new(Mutex::new(HashMap::new())),
            use_custom_loader: false,
            old_libs: Arc::new(Mutex::new(Vec::new())),
            symbol_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    #[cfg(target_os = "macos")]
    pub fn new_with_custom_loader() -> Self {
        Self {
            libs: Arc::new(Mutex::new(HashMap::new())),
            use_custom_loader: true,
            old_libs: Arc::new(Mutex::new(Vec::new())),
            symbol_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn load(&self, lib_path: &str) -> Result<String, Box<dyn std::error::Error>> {
        let lib_name = std::path::Path::new(lib_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid lib path")?
            .to_string();
        
        if self.use_custom_loader {
            // use custom mach-o loader
            #[cfg(target_os = "macos")]
            {
                // temporarily enable for debugging
                {
                    let load_start = std::time::Instant::now();
                    let mut loader = MachoLoader::new();
                    
                    unsafe {
                        loader.load(lib_path)?;
                    }
                    
                    let load_time = load_start.elapsed();
                    println!("{:.1}ms {}", load_time.as_secs_f64() * 1000.0, lib_path);
                    
                    let mut libs = self.libs.lock().unwrap();
                    
                    // If replacing an existing library, move it to old_libs instead of dropping
                    // This keeps the old code mapped to avoid TLV crashes
                    if let Some(old_lib) = libs.insert(lib_name.clone(), LoadedLibrary::Custom(Arc::new(Mutex::new(loader)))) {
                        let mut old_libs = self.old_libs.lock().unwrap();
                        old_libs.push(old_lib);
                    }

                    // Clear cached symbols for this library
                    self.symbol_cache
                        .lock()
                        .unwrap()
                        .retain(|(l, _), _| l != &lib_name);

                    return Ok(lib_name);
                }
            }
            
            #[cfg(not(target_os = "macos"))]
            return Err("custom loader only supported on macOS".into());
        }
        
        // use traditional dlopen
        let dlopen_start = std::time::Instant::now();
        
        // Use RTLD_LAZY for faster loading - symbols are resolved when first used
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let lib = unsafe {
            use libloading::os::unix::{Library as UnixLibrary, RTLD_LAZY};
            let unix_lib = UnixLibrary::open(Some(lib_path), RTLD_LAZY)?;
            Library::from(unix_lib)
        };
        
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        let lib = unsafe { Library::new(lib_path)? };
        
        let _dlopen_time = dlopen_start.elapsed();
        
        // dlopen timing check removed
        
        let mut libs = self.libs.lock().unwrap();

        // If replacing an existing library, move it to old_libs
        if let Some(old_lib) = libs.insert(lib_name.clone(), LoadedLibrary::Dlopen(Arc::new(lib))) {
            let mut old_libs = self.old_libs.lock().unwrap();
            old_libs.push(old_lib);
        }

        // Remove any cached symbols for this library
        self.symbol_cache
            .lock()
            .unwrap()
            .retain(|(l, _), _| l != &lib_name);

        Ok(lib_name)
    }

    pub fn with_symbol<T, R, F>(&self, lib_name: &str, symbol_name: &str, f: F) -> Result<R, Box<dyn std::error::Error>>
    where
        T: Sized + 'static,
        F: FnOnce(&Symbol<T>) -> R,
    {
        let cache_key = (lib_name.to_string(), symbol_name.to_string());

        if let Some(ptr) = self.symbol_cache.lock().unwrap().get(&cache_key).copied() {
            struct FakeSymbol<T> {
                ptr: *const T,
            }

            impl<T> std::ops::Deref for FakeSymbol<T> {
                type Target = *const T;
                fn deref(&self) -> &Self::Target {
                    &self.ptr
                }
            }

            let fake = FakeSymbol { ptr: ptr as *const T };
            let sym = &fake as *const FakeSymbol<T> as *const Symbol<T>;
            return Ok(f(unsafe { &*sym }));
        }

        // Get loaded library while holding the lock, then release the lock
        let loaded_lib = {
            let libs = self.libs.lock().unwrap();
            match libs.get(lib_name) {
                Some(lib) => match lib {
                    LoadedLibrary::Dlopen(arc) => LoadedLibrary::Dlopen(arc.clone()),
                    #[cfg(target_os = "macos")]
                    LoadedLibrary::Custom(arc) => LoadedLibrary::Custom(arc.clone()),
                },
                None => {
                    return Err(format!("library '{}' not loaded. Available: {:?}",
                        lib_name, libs.keys().collect::<Vec<_>>()).into());
                }
            }
        }; // mutex is dropped here

        match &loaded_lib {
            LoadedLibrary::Dlopen(lib_arc) => {
                let symbol: Symbol<T> = unsafe { lib_arc.get(symbol_name.as_bytes())? };
                let ptr = unsafe { symbol.clone().try_as_raw_ptr().unwrap() } as *const c_void;
                self.symbol_cache
                    .lock()
                    .unwrap()
                    .insert(cache_key, ptr);
                Ok(f(&symbol))
            }
            #[cfg(target_os = "macos")]
            LoadedLibrary::Custom(loader_arc) => {
                // For custom loader, we need to create a fake Symbol wrapper
                let loader = loader_arc.lock().unwrap();
                unsafe {
                    let addr = loader.get_symbol(symbol_name)
                        .ok_or_else(|| format!("symbol '{}' not found in custom loaded library", symbol_name))?;

                    // println!("    - Found symbol {} at address: {:p}", symbol_name, addr);

                    // Create a raw pointer to the function
                    let func_ptr = addr as *const T;
                    
                    // We can't create a real libloading::Symbol, so we pass the raw pointer
                    // wrapped in a way that matches the Symbol interface
                    // Symbol dereferences to *const T, so we create a wrapper that does the same
                    struct FakeSymbol<T> {
                        ptr: *const T,
                    }
                    
                    impl<T> std::ops::Deref for FakeSymbol<T> {
                        type Target = *const T;
                        fn deref(&self) -> &Self::Target {
                            &self.ptr
                        }
                    }
                    
                    let fake_symbol = FakeSymbol { ptr: func_ptr };

                    // cache symbol address
                    self.symbol_cache
                        .lock()
                        .unwrap()
                        .insert(cache_key, func_ptr as *const c_void);

                    // Since f expects &Symbol<T> but we have FakeSymbol<T>,
                    // and Symbol<T> derefs to *const T, we need to transmute
                    let symbol_ref = &fake_symbol as *const FakeSymbol<T> as *const Symbol<T>;
                    Ok(f(&*symbol_ref))
                }
            }
        }
    }

    pub fn call_constructor(
        &'static self,
        lib_name: &str,
        type_name: &str,
        rustc_commit: &str,
    ) -> Result<Box<dyn HotlineObject>, Box<dyn std::error::Error>> {
        // Set the library registry in thread-local storage before calling constructor
        // This allows the constructor (and any methods it calls) to create other objects
        crate::set_library_registry(self);
        
        let constructor_symbol = format!("{}__new____to__Box_lt_dyn_HotlineObject_gt__{}", type_name, rustc_commit);
        type ConstructorFn = fn() -> Box<dyn HotlineObject>;

        let loaded_lib = {
            let libs = self.libs.lock().unwrap();
            match libs.get(lib_name) {
                Some(lib) => match lib {
                    LoadedLibrary::Dlopen(arc) => LoadedLibrary::Dlopen(arc.clone()),
                    #[cfg(target_os = "macos")]
                    LoadedLibrary::Custom(arc) => LoadedLibrary::Custom(arc.clone()),
                },
                None => {
                    return Err(format!("library '{}' not loaded", lib_name).into());
                }
            }
        };

        let mut obj = match &loaded_lib {
            LoadedLibrary::Dlopen(_) => {
                self.with_symbol::<ConstructorFn, _, _>(lib_name, &constructor_symbol, |symbol| symbol())?
            }
            #[cfg(target_os = "macos")]
            LoadedLibrary::Custom(loader_arc) => {
                let loader = loader_arc.lock().unwrap();
                unsafe {
                    let addr = loader.get_symbol(&constructor_symbol)
                        .ok_or_else(|| format!("symbol '{}' not found", constructor_symbol))?;
                    let func: ConstructorFn = std::mem::transmute(addr);
                    func()
                }
            }
        };
        
        // Set the registry on the newly created object
        obj.set_registry(self);
        Ok(obj)
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
