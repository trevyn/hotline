use runtime::DirectRuntime;

fn main() -> Result<(), String> {
    use std::fs;
    use std::path::Path;
    
    // Leak the runtime to give it 'static lifetime
    let runtime = Box::leak(Box::new({
        #[cfg(target_os = "macos")]
        { DirectRuntime::new_with_custom_loader() }
        #[cfg(not(target_os = "macos"))]
        { DirectRuntime::new() }
    }));
    
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
                            eprintln!("Loading library: {}", lib_name);
                            runtime.hot_reload(&lib_path, lib_name)
                                .map_err(|e| format!("Failed to load {}: {}", lib_name, e))?;
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