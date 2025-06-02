use runtime::{DirectRuntime, direct_call};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::{Color, PixelFormatEnum};
use std::any::Any;
use std::time::Duration;

fn main() -> Result<(), String> {
    println!("Starting hotline runtime...");
    let sdl_context = sdl2::init()?;
    println!("SDL2 initialized");
    let video_subsystem = sdl_context.video()?;
    println!("Video subsystem initialized");

    let window = video_subsystem
        .window("hotline - direct calls", 800, 600)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;
    println!("Window created");

    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    println!("Canvas created");
    let texture_creator = canvas.texture_creator();
    println!("Texture creator initialized");
    let mut event_pump = sdl_context.event_pump()?;
    println!("Event pump created");

    let mut runtime = DirectRuntime::new();
    println!("DirectRuntime created");
    
    // Set the global library registry so objects can access it
    hotline::set_library_registry(runtime.library_registry().clone());
    println!("Library registry set globally");
    
    // Build and load WindowManager
    std::process::Command::new("cargo")
        .args(&["build", "--release", "-p", "WindowManager"])
        .status()
        .expect("Failed to build WindowManager");

    #[cfg(target_os = "macos")]
    let wm_path = "target/release/libWindowManager.dylib";
    #[cfg(target_os = "linux")]
    let wm_path = "target/release/libWindowManager.so";
    #[cfg(target_os = "windows")]
    let wm_path = "target/release/WindowManager.dll";

    println!("Loading WindowManager from: {}", wm_path);
    runtime.hot_reload(wm_path).expect("Failed to load WindowManager library");
    println!("WindowManager library loaded");

    // Create window manager instance
    println!("Creating WindowManager instance...");
    let window_manager = runtime.create_from_lib("libWindowManager", "WindowManager")
        .expect("Failed to create WindowManager");
    println!("WindowManager instance created");
    
    // Create texture once outside the loop
    println!("Creating streaming texture...");
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::ARGB8888, 800, 600)
        .map_err(|e| e.to_string())?;
    println!("Texture created successfully");
    
    println!("Entering main loop...");
    'running: loop {
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
                    println!("Reloading WindowManager library...");

                    // First rebuild the library
                    println!("Rebuilding WindowManager library...");
                    std::process::Command::new("cargo")
                        .args(&["build", "--release", "-p", "WindowManager"])
                        .status()
                        .expect("Failed to build WindowManager");

                    // Reload the runtime's copy
                    if let Err(e) = runtime.hot_reload(wm_path) {
                        eprintln!("Failed to reload WindowManager lib: {}", e);
                    }

                    println!("Reload complete!");
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

            // Call WindowManager render directly
            if let Ok(mut wm_guard) = window_manager.lock() {
                let wm_obj = &mut **wm_guard;
                let render_symbol = format!("WindowManager__render______obj_mut_dyn_Any____buffer__mut_ref_slice_u8_____buffer_width__i64_____buffer_height__i64_____pitch__i64____to__unit__{}", runtime::RUSTC_COMMIT);
                
                // Generic dynamic call
                type RenderFn = extern "Rust" fn(&mut dyn Any, &mut [u8], i64, i64, i64);
                match runtime.library_registry().with_symbol::<RenderFn, _, _>("libWindowManager", &render_symbol, |render_fn| {
                    let any_obj = wm_obj.as_any_mut();
                    unsafe { (**render_fn)(any_obj, buffer, 800, 600, pitch as i64) };
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

    println!("Exiting normally");
    Ok(())
}