use runtime::DirectRuntime;

fn main() -> Result<(), String> {
    use std::fs;
    use std::path::Path;
    
    // Ensure SDL2 libraries are loaded first
    #[cfg(target_os = "macos")]
    {
        const RTLD_LAZY: libc::c_int = 0x1;
        const RTLD_GLOBAL: libc::c_int = 0x8;
        
        unsafe {
            // Try homebrew paths first, then fallback to standard names
            let sdl2_paths = [
                "/opt/homebrew/lib/libSDL2-2.0.0.dylib",
                "/usr/local/lib/libSDL2-2.0.0.dylib",
                "libSDL2-2.0.0.dylib",
            ];
            
            let mut sdl2_loaded = false;
            for path in &sdl2_paths {
                let path_cstr = std::ffi::CString::new(*path).unwrap();
                let handle = libc::dlopen(path_cstr.as_ptr(), RTLD_LAZY | RTLD_GLOBAL);
                if !handle.is_null() {
                    eprintln!("Loaded SDL2 from: {}", path);
                    sdl2_loaded = true;
                    break;
                }
            }
            if !sdl2_loaded {
                eprintln!("Warning: Failed to load SDL2 library");
            }
            
            // Load SDL2_ttf
            let ttf_paths = [
                "/opt/homebrew/lib/libSDL2_ttf-2.0.0.dylib",
                "/usr/local/lib/libSDL2_ttf-2.0.0.dylib",
                "libSDL2_ttf-2.0.0.dylib",
            ];
            
            let mut ttf_loaded = false;
            for path in &ttf_paths {
                let path_cstr = std::ffi::CString::new(*path).unwrap();
                let handle = libc::dlopen(path_cstr.as_ptr(), RTLD_LAZY | RTLD_GLOBAL);
                if !handle.is_null() {
                    eprintln!("Loaded SDL2_ttf from: {}", path);
                    ttf_loaded = true;
                    break;
                }
            }
            if !ttf_loaded {
                eprintln!("Warning: Failed to load SDL2_ttf library");
            }
            
            // Load SDL2_image
            let image_paths = [
                "/opt/homebrew/lib/libSDL2_image-2.0.0.dylib",
                "/usr/local/lib/libSDL2_image-2.0.0.dylib",
                "libSDL2_image-2.0.0.dylib",
            ];
            
            let mut image_loaded = false;
            for path in &image_paths {
                let path_cstr = std::ffi::CString::new(*path).unwrap();
                let handle = libc::dlopen(path_cstr.as_ptr(), RTLD_LAZY | RTLD_GLOBAL);
                if !handle.is_null() {
                    eprintln!("Loaded SDL2_image from: {}", path);
                    image_loaded = true;
                    break;
                }
            }
            if !image_loaded {
                eprintln!("Warning: Failed to load SDL2_image library");
            }
        }
    }
    
    // Leak the runtime to give it 'static lifetime
    let runtime = Box::leak(Box::new({
        #[cfg(target_os = "macos")]
        { DirectRuntime::new_with_custom_loader() }
        #[cfg(not(target_os = "macos"))]
        { DirectRuntime::new() }
    }));
    
    // Start watching objects directory for changes
    runtime.start_watching("objects")
        .map_err(|e| format!("Failed to start file watcher: {}", e))?;
    eprintln!("Started watching objects/ directory for changes");
    
    // Load all libraries from objects directory
    let objects_dir = Path::new("objects");
    if let Ok(entries) = fs::read_dir(objects_dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(lib_name) = path.file_name().and_then(|n| n.to_str()) {
                        #[cfg(target_os = "macos")]
                        let lib_path = format!("target/release/lib{}.dylib", lib_name);
                        #[cfg(target_os = "linux")]
                        let lib_path = format!("target/release/lib{}.so", lib_name);
                        #[cfg(target_os = "windows")]
                        let lib_path = format!("target/release/{}.dll", lib_name);
                        
                        if Path::new(&lib_path).exists() {
                            eprintln!("Loading library: {} from {}", lib_name, lib_path);
                            runtime.hot_reload(&lib_path, lib_name)
                                .map_err(|e| format!("Failed to load {}: {}", lib_name, e))?;
                        } else {
                            eprintln!("Library not found: {}", lib_path);
                        }
                    }
                }
            }
        }
    }
    
    // Now create Application
    let app_handle = runtime.create_from_lib("libApplication", "Application")
        .map_err(|e| e.to_string())?;
    
    // Get the Application object and call run
    if let Ok(mut app_guard) = app_handle.lock() {
        let app = &mut **app_guard;
        
        // Call initialize method
        let init_symbol = format!(
            "Application__initialize______obj_mut_dyn_Any____to__Result_lt_unit_String_gt__{}",
            runtime::RUSTC_COMMIT
        );
        type InitFn = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> Result<(), String>;
        runtime.library_registry().with_symbol::<InitFn, _, _>(
            "libApplication",
            &init_symbol,
            |init_fn| unsafe { (**init_fn)(app.as_any_mut()) }
        ).map_err(|e| e.to_string())??;
        
        // Call run method
        let run_symbol = format!(
            "Application__run______obj_mut_dyn_Any____to__Result_lt_unit_String_gt__{}",
            runtime::RUSTC_COMMIT
        );
        type RunFn = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> Result<(), String>;
        runtime.library_registry().with_symbol::<RunFn, _, _>(
            "libApplication",
            &run_symbol,
            |run_fn| unsafe { (**run_fn)(app.as_any_mut()) }
        ).map_err(|e| e.to_string())??;
    }
    
    Ok(())
}