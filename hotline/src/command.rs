use crate::HotlineObject;
#[cfg(target_os = "macos")]
use crate::macho_loader::MachoLoader;
use libloading::{Library, Symbol};
use std::any::Any;
use std::collections::HashMap;
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
}

impl LibraryRegistry {
    pub fn new() -> Self {
        Self {
            libs: Arc::new(Mutex::new(HashMap::new())),
            use_custom_loader: false,
            old_libs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn new_with_custom_loader() -> Self {
        Self {
            libs: Arc::new(Mutex::new(HashMap::new())),
            use_custom_loader: true,
            old_libs: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[cfg(target_os = "macos")]
    fn load_dependencies(&self, dependencies: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        use std::collections::HashSet;
        use std::ffi::CString;

        const RTLD_LAZY: libc::c_int = 0x1;
        const RTLD_GLOBAL: libc::c_int = 0x8;

        if dependencies.is_empty() {
            return Ok(());
        }

        // Track what we've already loaded
        static LOADED_DEPS: std::sync::OnceLock<Mutex<HashSet<String>>> = std::sync::OnceLock::new();
        let loaded = LOADED_DEPS.get_or_init(|| Mutex::new(HashSet::new()));

        for dep in dependencies {
            // Skip system libraries and already loaded deps
            if dep.starts_with("/usr/lib/") || dep.starts_with("/System/") {
                continue;
            }

            // Extract library name from path
            let lib_name = dep.split('/').last().unwrap_or(dep);

            // Check if already loaded
            {
                let mut loaded_set = loaded.lock().unwrap();
                if loaded_set.contains(lib_name) {
                    continue;
                }
                loaded_set.insert(lib_name.to_string());
            }

            // Try different paths for the dependency
            let mut paths = vec![];

            // If it's an @rpath dependency, try common locations
            if dep.starts_with("@rpath/") {
                let lib_name = dep.strip_prefix("@rpath/").unwrap();
                paths.push(format!("/opt/homebrew/lib/{}", lib_name));
                paths.push(format!("/usr/local/lib/{}", lib_name));
            } else if !dep.starts_with('/') {
                // Relative path - try common locations
                paths.push(format!("/opt/homebrew/lib/{}", dep));
                paths.push(format!("/usr/local/lib/{}", dep));
            } else {
                // Absolute path
                paths.push(dep.clone());
            }

            // Also try without version suffix
            // e.g., libSDL2-2.0.0.dylib -> libSDL2.dylib
            if lib_name.contains('-') {
                if let Some(base_name) = lib_name.split('-').next() {
                    paths.push(format!("/opt/homebrew/lib/{}.dylib", base_name));
                    paths.push(format!("/usr/local/lib/{}.dylib", base_name));
                }
            }

            // Try to load from each path
            let mut loaded = false;
            for path in &paths {
                let path_cstr = CString::new(path.as_str())?;
                let dep_start = std::time::Instant::now();
                let handle = unsafe { libc::dlopen(path_cstr.as_ptr(), RTLD_LAZY | RTLD_GLOBAL) };
                if !handle.is_null() {
                    let elapsed = dep_start.elapsed();
                    eprintln!("  {:.1}ms {}", elapsed.as_secs_f64() * 1000.0, path);
                    loaded = true;
                    break;
                }
            }

            if !loaded {
                eprintln!("Warning: Failed to load dependency: {} (tried: {:?})", lib_name, paths);
            }
        }

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    fn load_dependencies(&self, _dependencies: &[String]) -> Result<(), Box<dyn std::error::Error>> {
        // On non-macOS platforms, dependencies are handled by the system loader
        Ok(())
    }

    pub fn load(&self, lib_path: &str) -> Result<String, Box<dyn std::error::Error>> {
        let lib_name =
            std::path::Path::new(lib_path).file_stem().and_then(|s| s.to_str()).ok_or("invalid lib path")?.to_string();

        if self.use_custom_loader {
            // use custom mach-o loader
            #[cfg(target_os = "macos")]
            {
                // temporarily enable for debugging
                {
                    let load_start = std::time::Instant::now();
                    let mut loader = MachoLoader::new();

                    // First, parse dependencies
                    let dependencies = unsafe { loader.parse_dependencies(lib_path)? };

                    // Load dependencies BEFORE finishing the load
                    // This ensures symbols are available when processing lazy bindings
                    self.load_dependencies(&dependencies)?;

                    // Now finish loading with dependencies available
                    unsafe {
                        loader.finish_loading()?;
                    }

                    let load_time = load_start.elapsed();
                    println!("{:.1}ms {}", load_time.as_secs_f64() * 1000.0, lib_path);

                    let mut libs = self.libs.lock().unwrap();

                    // If replacing an existing library, move it to old_libs instead of dropping
                    // This keeps the old code mapped to avoid TLV crashes
                    if let Some(old_lib) =
                        libs.insert(lib_name.clone(), LoadedLibrary::Custom(Arc::new(Mutex::new(loader))))
                    {
                        let mut old_libs = self.old_libs.lock().unwrap();
                        old_libs.push(old_lib);
                    }

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

        Ok(lib_name)
    }

    pub fn with_symbol<T, R, F>(&self, lib_name: &str, symbol_name: &str, f: F) -> Result<R, Box<dyn std::error::Error>>
    where
        T: 'static,
        F: FnOnce(&Symbol<T>) -> R,
    {
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
                    return Err(format!(
                        "library '{}' not loaded. Available: {:?}",
                        lib_name,
                        libs.keys().collect::<Vec<_>>()
                    )
                    .into());
                }
            }
        }; // mutex is dropped here

        match &loaded_lib {
            LoadedLibrary::Dlopen(lib_arc) => {
                let symbol: Symbol<T> = unsafe { lib_arc.get(symbol_name.as_bytes())? };
                Ok(f(&symbol))
            }
            #[cfg(target_os = "macos")]
            LoadedLibrary::Custom(loader_arc) => {
                // For custom loader, we need to create a fake Symbol wrapper
                let loader = loader_arc.lock().unwrap();
                unsafe {
                    let addr = loader.get_symbol(symbol_name).ok_or_else(|| {
                        // Get all available symbols for better error reporting
                        let available_symbols = loader
                            .list_symbols()
                            .into_iter()
                            .filter(|s| s.contains(&symbol_name[..symbol_name.len().min(20)]))
                            .take(10)
                            .collect::<Vec<_>>();

                        if available_symbols.is_empty() {
                            format!("symbol '{}' not found in custom loaded library", symbol_name)
                        } else {
                            format!(
                                "symbol '{}' not found in custom loaded library. Similar symbols:\n{}",
                                symbol_name,
                                available_symbols.join("\n")
                            )
                        }
                    })?;

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
                    let addr = loader
                        .get_symbol(&constructor_symbol)
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
