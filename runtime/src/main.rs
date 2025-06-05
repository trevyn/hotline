use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use runtime::{DirectRuntime, direct_call};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::{Color, PixelFormatEnum};
use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{TryRecvError, channel};
use std::time::Duration;
use xxhash_rust::xxh3::xxh3_64;

fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window("hotline - direct calls", 800, 600)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();
    let mut event_pump = sdl_context.event_pump()?;

    // Leak the runtime to give it 'static lifetime so objects can store references to it
    let runtime = Box::leak(Box::new({
        #[cfg(target_os = "macos")]
        { DirectRuntime::new_with_custom_loader() }
        #[cfg(not(target_os = "macos"))]
        { DirectRuntime::new() }
    }));

    // Dynamically discover and load libraries from objects directory
    use std::fs;
    use std::path::Path;

    let objects_dir = Path::new("objects");
    let mut loaded_libs = Vec::new();

    // First, rebuild all libraries at launch
    // Rebuilding all libraries at launch
    if let Ok(entries) = fs::read_dir(objects_dir) {
        let lib_names: Vec<String> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if !path.is_dir() {
                    return None;
                }
                path.file_name()?.to_str().map(|s| s.to_string())
            })
            .collect();
        
        if !lib_names.is_empty() {
            // Building libraries in parallel
            let mut cmd = std::process::Command::new("cargo");
            cmd.args(&["build", "--release"]);
            
            // add all packages
            for lib_name in &lib_names {
                cmd.args(&["-p", lib_name]);
            }
            
            // use status() instead of output() to see real-time output
            let status = cmd.status().expect("failed to build libraries");
            
            if !status.success() {
                panic!("failed to build libraries");
            }
        }
    }

    // Load all libraries
    if let Ok(entries) = fs::read_dir(objects_dir) {
        let libs_to_load: Vec<(String, String)> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if !path.is_dir() {
                    return None;
                }
                let lib_name = path.file_name()?.to_str()?.to_string();
                
                // Construct library path based on OS
                #[cfg(target_os = "macos")]
                let lib_path = format!("target/release/lib{}.dylib", lib_name);
                #[cfg(target_os = "linux")]
                let lib_path = format!("target/release/lib{}.so", lib_name);
                #[cfg(target_os = "windows")]
                let lib_path = format!("target/release/{}.dll", lib_name);
                
                if Path::new(&lib_path).exists() {
                    Some((lib_name, lib_path))
                } else {
                    eprintln!("Library not found at {}, skipping", lib_path);
                    None
                }
            })
            .collect();
        
        // Load libraries sequentially (dlopen can be finicky with parallelism)
        // Loading libraries
        let start = std::time::Instant::now();
        
        for (lib_name, lib_path) in libs_to_load {
            if let Err(e) = runtime.hot_reload(&lib_path, &lib_name) {
                eprintln!("Failed to load {} library: {}", lib_name, e);
            } else {
                loaded_libs.push((lib_name, lib_path));
            }
        }
        
        let total_elapsed = start.elapsed();
        println!("Total loading time: {:.1}ms", total_elapsed.as_secs_f64() * 1000.0);
    }

    // Store lib paths for hot reload
    let lib_paths = loaded_libs.clone();

    // Set up file watcher for automatic hot reload
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default()).expect("Failed to create file watcher");

    // Watch lib.rs files in each object directory and compute initial hashes
    let mut file_hashes: HashMap<String, u64> = HashMap::new();
    for (lib_name, _) in &loaded_libs {
        let lib_rs_path = format!("objects/{}/src/lib.rs", lib_name);
        if Path::new(&lib_rs_path).exists() {
            watcher
                .watch(Path::new(&lib_rs_path), RecursiveMode::NonRecursive)
                .expect(&format!("Failed to watch {}", lib_rs_path));
            // Watching for changes

            // Compute initial hash
            if let Ok(contents) = std::fs::read(&lib_rs_path) {
                let hash = xxh3_64(&contents);
                file_hashes.insert(lib_name.clone(), hash);
            }
        }
    }

    // Create window manager instance
    let window_manager =
        runtime.create_from_lib("libWindowManager", "WindowManager").expect("Failed to create WindowManager");

    // Initialize window manager (which sets up the text renderer)
    direct_call!(runtime, &window_manager, WindowManager, initialize()).expect("Failed to initialize WindowManager");

    // Create texture once outside the loop
    let mut texture =
        texture_creator.create_texture_streaming(PixelFormatEnum::ARGB8888, 800, 600).map_err(|e| e.to_string())?;

    'running: loop {
        // Check for file system events
        match rx.try_recv() {
            Ok(event) => {
                if let Ok(event) = event {
                    // Find which library changed
                    for (lib_name, lib_path) in &lib_paths {
                        let lib_rs_path = format!("objects/{}/src/lib.rs", lib_name);
                        let lib_rs_pathbuf = PathBuf::from(&lib_rs_path);

                        if event.paths.iter().any(|p| p.ends_with(&lib_rs_pathbuf)) {
                            // Read file and compute hash
                            if let Ok(contents) = std::fs::read(&lib_rs_path) {
                                let new_hash = xxh3_64(&contents);
                                let old_hash = file_hashes.get(lib_name).copied().unwrap_or(0);

                                if new_hash != old_hash {
                                    // Detected change, rebuilding and reloading

                                    // Update hash
                                    file_hashes.insert(lib_name.clone(), new_hash);

                                    // Rebuild the specific library
                                    let status = std::process::Command::new("cargo")
                                        .args(&["build", "--release", "-p", lib_name])
                                        .status()
                                        .expect(&format!("Failed to build {}", lib_name));
                                    
                                    if !status.success() {
                                        eprintln!("Failed to build {}", lib_name);
                                        continue;
                                    }

                                    // Reload the library
                                    if let Err(e) = runtime.hot_reload(lib_path, lib_name) {
                                        eprintln!("Failed to reload {} lib: {}", lib_name, e);
                                    } else {
                                        // Successfully reloaded
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                eprintln!("File watcher disconnected");
            }
        }

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running;
                }
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } => {
                    // Pass event to WindowManager
                    direct_call!(runtime, &window_manager, WindowManager, handle_mouse_down(x as f64, y as f64))
                        .expect("Failed to handle mouse down");
                }
                Event::MouseButtonUp { mouse_btn: MouseButton::Left, x, y, .. } => {
                    // Pass event to WindowManager
                    direct_call!(runtime, &window_manager, WindowManager, handle_mouse_up(x as f64, y as f64))
                        .expect("Failed to handle mouse up");
                }
                Event::MouseMotion { x, y, .. } => {
                    // Pass event to WindowManager
                    direct_call!(runtime, &window_manager, WindowManager, handle_mouse_motion(x as f64, y as f64))
                        .expect("Failed to handle mouse motion");
                }
                // Hot reload on R key
                Event::KeyDown { keycode: Some(Keycode::R), .. } => {
                    // Build all libraries in parallel with single cargo invocation
                    let mut cmd = std::process::Command::new("cargo");
                    cmd.args(&["build", "--release"]);
                    
                    for (lib_name, _) in &lib_paths {
                        cmd.args(&["-p", lib_name]);
                    }
                    
                    let status = cmd.status().expect("Failed to build libraries");
                    if !status.success() {
                        eprintln!("Failed to rebuild libraries");
                        continue;
                    }

                    // Reload all libraries
                    for (lib_name, lib_path) in &lib_paths {
                        if let Err(e) = runtime.hot_reload(lib_path, lib_name) {
                            eprintln!("Failed to reload {} lib: {}", lib_name, e);
                        }
                    }
                }
                _ => {}
            }
        }

        // Render to texture
        texture.with_lock(None, |buffer: &mut [u8], pitch: usize| {
            // Clear buffer with a dark gray color
            for pixel in buffer.chunks_exact_mut(4) {
                pixel[0] = 30;  // B
                pixel[1] = 30;  // G
                pixel[2] = 30;  // R
                pixel[3] = 255; // A
            }

            // First render the WindowManager (which will try to render rects but fail due to registry access)
            if let Ok(mut wm_guard) = window_manager.lock() {
                let wm_obj = &mut **wm_guard;
                let render_symbol = format!("WindowManager__render______obj_mut_dyn_Any____buffer__mut_ref_slice_u8____buffer_width__i64____buffer_height__i64____pitch__i64____to__unit__{}", runtime::RUSTC_COMMIT);

                // Generic dynamic call
                type RenderFn = extern "Rust" fn(&mut dyn Any, &mut [u8], i64, i64, i64);
                match runtime.library_registry().with_symbol::<RenderFn, _, _>("libWindowManager", &render_symbol, |render_fn| {
                    let any_obj = wm_obj.as_any_mut();
                    (**render_fn)(any_obj, buffer, 800, 600, pitch as i64);
                }) {
                    Ok(_) => {},
                    Err(e) => {
                        eprintln!("Failed to get render symbol: {}", e);
                    }
                }
            }

        })?;

        // Copy texture to canvas
        canvas.copy(&texture, None, None)?;

        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
