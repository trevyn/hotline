use runtime::{DirectRuntime, direct_call};
use hotline::ObjectHandle;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::{Color, PixelFormatEnum};
use std::any::Any;
use std::time::Duration;

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

    let mut runtime = DirectRuntime::new();
    
    // Build and load Rect first (dependency of WindowManager)
    std::process::Command::new("cargo")
        .args(&["build", "--release", "-p", "rect"])
        .status()
        .expect("Failed to build rect");

    #[cfg(target_os = "macos")]
    let rect_path = "target/release/librect.dylib";
    #[cfg(target_os = "linux")]
    let rect_path = "target/release/librect.so";
    #[cfg(target_os = "windows")]
    let rect_path = "target/release/rect.dll";

    runtime.hot_reload(rect_path, "Rect").expect("Failed to load rect library");

    // Build and load HighlightLens
    std::process::Command::new("cargo")
        .args(&["build", "--release", "-p", "HighlightLens"])
        .status()
        .expect("Failed to build HighlightLens");

    #[cfg(target_os = "macos")]
    let hl_path = "target/release/libHighlightLens.dylib";
    #[cfg(target_os = "linux")]
    let hl_path = "target/release/libHighlightLens.so";
    #[cfg(target_os = "windows")]
    let hl_path = "target/release/HighlightLens.dll";

    runtime.hot_reload(hl_path, "HighlightLens").expect("Failed to load HighlightLens library");

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

    runtime.hot_reload(wm_path, "WindowManager").expect("Failed to load WindowManager library");

    // Create window manager instance
    let window_manager = runtime.create_from_lib("libWindowManager", "WindowManager")
        .expect("Failed to create WindowManager");
    
    
    // Create texture once outside the loop
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::ARGB8888, 800, 600)
        .map_err(|e| e.to_string())?;
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
                    // First rebuild the libraries
                    std::process::Command::new("cargo")
                        .args(&["build", "--release", "-p", "rect"])
                        .status()
                        .expect("Failed to build rect");
                        
                    std::process::Command::new("cargo")
                        .args(&["build", "--release", "-p", "HighlightLens"])
                        .status()
                        .expect("Failed to build HighlightLens");
                        
                    std::process::Command::new("cargo")
                        .args(&["build", "--release", "-p", "WindowManager"])
                        .status()
                        .expect("Failed to build WindowManager");

                    // Reload all libraries
                    if let Err(e) = runtime.hot_reload(rect_path, "Rect") {
                        eprintln!("Failed to reload rect lib: {}", e);
                    }
                    if let Err(e) = runtime.hot_reload(hl_path, "HighlightLens") {
                        eprintln!("Failed to reload HighlightLens lib: {}", e);
                    }
                    if let Err(e) = runtime.hot_reload(wm_path, "WindowManager") {
                        eprintln!("Failed to reload WindowManager lib: {}", e);
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

    Ok(())
}