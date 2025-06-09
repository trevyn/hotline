use runtime::DirectRuntime;
use runtime::compile_all_objects;

fn main() -> Result<(), String> {
    use std::fs;
    use std::path::Path;

    // Dependencies will be loaded automatically by the custom macho loader

    // Leak the runtime to give it 'static lifetime
    let runtime = Box::leak(Box::new({
        #[cfg(target_os = "macos")]
        {
            DirectRuntime::new_with_custom_loader()
        }
        #[cfg(not(target_os = "macos"))]
        {
            DirectRuntime::new()
        }
    }));

    // Start watching objects directory for changes
    runtime.start_watching("objects").map_err(|e| format!("Failed to start file watcher: {}", e))?;
    eprintln!("Started watching objects/ directory for changes");

    // Compile all object crates once at startup
    compile_all_objects(Path::new("objects")).map_err(|e| format!("Failed to compile objects: {}", e))?;

    // Load all libraries from objects directory
    let objects_dir = Path::new("objects");
    let load_start = std::time::Instant::now();
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
                            // Loading library
                            if let Err(e) = runtime.hot_reload(&lib_path, lib_name) {
                                #[cfg(target_os = "linux")]
                                {
                                    if e.to_string().contains("libSDL3") {
                                        eprintln!("Warning: {}", e);
                                        continue;
                                    }
                                }
                                return Err(format!("Failed to load {}: {}", lib_name, e));
                            }
                        } else {
                            eprintln!("Library not found: {}", lib_path);
                        }
                    }
                }
            }
        }
    }
    let load_time = load_start.elapsed();
    eprintln!("------\n{:.1}ms Total library loading time", load_time.as_secs_f64() * 1000.0);

    // Now create Application
    let app_handle = match runtime.create_from_lib("libApplication", "Application") {
        Ok(handle) => handle,
        Err(e) => {
            #[cfg(target_os = "linux")]
            {
                if e.to_string().contains("not loaded") {
                    eprintln!("Warning: {}", e);
                    return Ok(());
                }
            }
            return Err(e.to_string());
        }
    };

    // Get the Application object and call run
    if let Ok(mut app_guard) = app_handle.lock() {
        let app = &mut **app_guard;

        // Call initialize method
        let init_symbol = format!(
            "Application__initialize______obj_mut_dyn_Any____to__Result_lt_unit_String_gt__{}",
            runtime::RUSTC_COMMIT
        );
        type InitFn = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> Result<(), String>;
        runtime
            .library_registry()
            .with_symbol::<InitFn, _, _>("libApplication", &init_symbol, |init_fn| unsafe {
                (**init_fn)(app.as_any_mut())
            })
            .map_err(|e| e.to_string())??;

        // Call run method
        let run_symbol =
            format!("Application__run______obj_mut_dyn_Any____to__Result_lt_unit_String_gt__{}", runtime::RUSTC_COMMIT);
        type RunFn = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> Result<(), String>;
        runtime
            .library_registry()
            .with_symbol::<RunFn, _, _>("libApplication", &run_symbol, |run_fn| unsafe { (**run_fn)(app.as_any_mut()) })
            .map_err(|e| e.to_string())??;
    }

    Ok(())
}
