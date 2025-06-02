use hotline::ObjectHandle;
use runtime::{DirectRuntime, direct_call};
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

    // Build and load WindowManager
    println!("Building WindowManager library...");
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

    runtime.hot_reload(wm_path).expect("Failed to load WindowManager library");

    // Create window manager instance
    let window_manager = runtime.create_from_lib("libWindowManager", "WindowManager")
        .expect("Failed to create WindowManager");

    let lib_path = {
        #[cfg(target_os = "macos")]
        let path = "target/release/librect.dylib";
        #[cfg(target_os = "linux")]
        let path = "target/release/librect.so";
        #[cfg(target_os = "windows")]
        let path = "target/release/rect.dll";

        // First build the rect library
        println!("Building rect library...");
        std::process::Command::new("cargo")
            .args(&["build", "--release", "-p", "rect"])
            .status()
            .expect("Failed to build rect");

        runtime.hot_reload(path).expect("Failed to load rect library");
        path
    };

    let mut render_lib = unsafe { libloading::Library::new(lib_path) }.expect("Failed to load lib");
    let render_symbol = format!("Rect__render______obj_mut_dyn_Any____buffer__unknown____buffer_width__i64____buffer_height__i64____pitch__i64____to__unit__{}", runtime::RUSTC_COMMIT);
    let mut render_rect: libloading::Symbol<
        unsafe extern "Rust" fn(&mut dyn Any, &mut [u8], i64, i64, i64),
    > = unsafe { render_lib.get(render_symbol.as_bytes()) }.expect("Failed to find render function");

    let mut drag_start = None;

    'running: loop {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running;
                }
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } => {
                    println!("Click at ({}, {})", x, y);
                    
                    // Check if clicking on existing rect through WindowManager
                    let mut hit = false;
                    let rects_count = direct_call!(runtime, window_manager, WindowManager, get_rects_count())
                        .expect("Failed to get rects count") as usize;
                    
                    for i in (0..rects_count).rev() {
                        let handle_id = direct_call!(runtime, window_manager, WindowManager, get_rect_at(i as i64))
                            .expect("Failed to get rect at index");
                        
                        if handle_id >= 0 {
                            let rect_handle = ObjectHandle(handle_id as u64);
                            
                            // Get bounds using getter methods
                            let rect_x: f64 = direct_call!(runtime, rect_handle, Rect, x())
                                .expect("Failed to get x");
                            let rect_y: f64 = direct_call!(runtime, rect_handle, Rect, y())
                                .expect("Failed to get y");
                            let rect_width: f64 = direct_call!(runtime, rect_handle, Rect, width())
                                .expect("Failed to get width");
                            let rect_height: f64 = direct_call!(runtime, rect_handle, Rect, height())
                                .expect("Failed to get height");

                            if x as f64 >= rect_x
                                && x as f64 <= rect_x + rect_width
                                && y as f64 >= rect_y
                                && y as f64 <= rect_y + rect_height
                            {
                                println!("  HIT! Selected rect");
                                direct_call!(runtime, window_manager, WindowManager, start_dragging(rect_handle))
                                    .expect("Failed to start dragging");
                                direct_call!(runtime, window_manager, WindowManager, set_drag_offset(x as f64 - rect_x, y as f64 - rect_y))
                                    .expect("Failed to set drag offset");
                                hit = true;
                                break;
                            }
                        }
                    }

                    if !hit {
                        println!("  No hit, starting new rect creation");
                        direct_call!(runtime, window_manager, WindowManager, clear_selection())
                            .expect("Failed to clear selection");
                        drag_start = Some((x, y));
                    }
                }
                Event::MouseButtonUp { mouse_btn: MouseButton::Left, x, y, .. } => {
                    let is_dragging = direct_call!(runtime, window_manager, WindowManager, is_dragging())
                        .expect("Failed to check dragging state");
                        
                    if is_dragging {
                        // Stop dragging
                        direct_call!(runtime, window_manager, WindowManager, stop_dragging())
                            .expect("Failed to stop dragging");
                    } else if let Some((start_x, start_y)) = drag_start {
                        // Create new rect
                        let box_x = start_x.min(x) as f64;
                        let box_y = start_y.min(y) as f64;
                        let box_w = (start_x - x).abs() as f64;
                        let box_h = (start_y - y).abs() as f64;

                        if box_w > 0.0 && box_h > 0.0 {
                            let handle = runtime.create_from_lib("librect", "Rect")
                                .expect("Failed to create rect");

                            // Set initial properties
                            direct_call!(runtime, handle, Rect, set_x(box_x)).expect("Failed to set x");
                            direct_call!(runtime, handle, Rect, set_y(box_y)).expect("Failed to set y");
                            direct_call!(runtime, handle, Rect, set_width(box_w)).expect("Failed to set width");
                            direct_call!(runtime, handle, Rect, set_height(box_h)).expect("Failed to set height");

                            println!(
                                "Created rect with bounds: ({}, {}, {}, {})",
                                box_x, box_y, box_w, box_h
                            );
                            
                            // Add to window manager
                            direct_call!(runtime, window_manager, WindowManager, add_rect(handle))
                                .expect("Failed to add rect to window manager");
                        }
                        drag_start = None;
                    }
                }
                Event::MouseMotion { x, y, .. } => {
                    let is_dragging = direct_call!(runtime, window_manager, WindowManager, is_dragging())
                        .expect("Failed to check dragging state");
                        
                    if is_dragging {
                        let selected_handle_id = direct_call!(runtime, window_manager, WindowManager, get_selected_handle())
                            .expect("Failed to get selected handle");
                            
                        if selected_handle_id >= 0 {
                            let handle = ObjectHandle(selected_handle_id as u64);
                            
                            // Get drag offset
                            let drag_offset_x = direct_call!(runtime, window_manager, WindowManager, drag_offset_x())
                                .expect("Failed to get drag offset x");
                            let drag_offset_y = direct_call!(runtime, window_manager, WindowManager, drag_offset_y())
                                .expect("Failed to get drag offset y");
                            
                            // Get current position to calculate delta
                            let rect_x: f64 = direct_call!(runtime, handle, Rect, x())
                                .expect("Failed to get x for dragging");
                            let rect_y: f64 = direct_call!(runtime, handle, Rect, y())
                                .expect("Failed to get y for dragging");

                            let new_x = x as f64 - drag_offset_x;
                            let new_y = y as f64 - drag_offset_y;
                            let dx = new_x - rect_x;
                            let dy = new_y - rect_y;

                            // Move the rect
                            direct_call!(runtime, handle, Rect, move_by(dx, dy)).expect("Failed to move rect");
                        }
                    }
                }
                // Hot reload on R key
                Event::KeyDown { keycode: Some(Keycode::R), .. } => {
                    println!("Reloading rect library...");

                    // First rebuild the library
                    println!("Rebuilding rect library...");
                    std::process::Command::new("cargo")
                        .args(&["build", "--release", "-p", "rect"])
                        .status()
                        .expect("Failed to build rect");

                    // Reload the runtime's copy
                    if let Err(e) = runtime.hot_reload(lib_path) {
                        eprintln!("Failed to reload runtime lib: {}", e);
                    }

                    // Reload our render function
                    drop(render_rect);
                    drop(render_lib);

                    // Small delay to ensure file is ready
                    std::thread::sleep(Duration::from_millis(100));

                    render_lib = unsafe { libloading::Library::new(lib_path) }
                        .expect("Failed to reload render lib");
                    render_rect = unsafe { render_lib.get(render_symbol.as_bytes()) }
                        .expect("Failed to reload render function");

                    println!("Reload complete!");
                }
                _ => {}
            }
        }

        // Create texture and render rects to it
        let mut texture = texture_creator
            .create_texture_streaming(PixelFormatEnum::ARGB8888, 800, 600)
            .map_err(|e| e.to_string())?;

        texture.with_lock(None, |buffer: &mut [u8], pitch: usize| {
            // Clear buffer
            for byte in buffer.iter_mut() {
                *byte = 0;
            }

            // Render rects to buffer
            let rects_count = direct_call!(runtime, window_manager, WindowManager, get_rects_count())
                .unwrap_or(0) as usize;
                
            for i in 0..rects_count {
                let handle_id = direct_call!(runtime, window_manager, WindowManager, get_rect_at(i as i64))
                    .unwrap_or(-1);
                    
                if handle_id >= 0 {
                    let rect_handle = ObjectHandle(handle_id as u64);
                    if let Some(rect_obj) = runtime.get_object_mut(rect_handle) {
                        unsafe {
                            render_rect(rect_obj, buffer, 800, 600, pitch as i64);
                        }
                    }
                }
            }

            // Highlight selected rect with a border
            let selected_handle_id = direct_call!(runtime, window_manager, WindowManager, get_selected_handle())
                .unwrap_or(-1);
                
            if selected_handle_id >= 0 {
                let sel_handle = ObjectHandle(selected_handle_id as u64);
                let rect_x = direct_call!(runtime, sel_handle, Rect, x())
                    .unwrap_or(0.0);
                let rect_y = direct_call!(runtime, sel_handle, Rect, y())
                    .unwrap_or(0.0);
                let rect_width = direct_call!(runtime, sel_handle, Rect, width())
                    .unwrap_or(0.0);
                let rect_height = direct_call!(runtime, sel_handle, Rect, height())
                    .unwrap_or(0.0);

                // Draw selection border
                let x_start = (rect_x as i32).max(0) as u32;
                let y_start = (rect_y as i32).max(0) as u32;
                let x_end = ((rect_x + rect_width) as i32).min(800) as u32;
                let y_end = ((rect_y + rect_height) as i32).min(600) as u32;

                // Top and bottom borders
                for x in x_start..x_end {
                    let top_offset = (y_start * (pitch as u32) + x * 4) as usize;
                    let bottom_offset = (((y_end - 1) * (pitch as u32)) + x * 4) as usize;
                    if top_offset + 3 < buffer.len() {
                        buffer[top_offset] = 0; // B
                        buffer[top_offset + 1] = 255; // G
                        buffer[top_offset + 2] = 0; // R
                        buffer[top_offset + 3] = 255; // A
                    }
                    if bottom_offset + 3 < buffer.len() {
                        buffer[bottom_offset] = 0; // B
                        buffer[bottom_offset + 1] = 255; // G
                        buffer[bottom_offset + 2] = 0; // R
                        buffer[bottom_offset + 3] = 255; // A
                    }
                }

                // Left and right borders
                for y in y_start..y_end {
                    let left_offset = (y * (pitch as u32) + x_start * 4) as usize;
                    let right_offset = (y * (pitch as u32) + (x_end - 1) * 4) as usize;
                    if left_offset + 3 < buffer.len() {
                        buffer[left_offset] = 0; // B
                        buffer[left_offset + 1] = 255; // G
                        buffer[left_offset + 2] = 0; // R
                        buffer[left_offset + 3] = 255; // A
                    }
                    if right_offset + 3 < buffer.len() {
                        buffer[right_offset] = 0; // B
                        buffer[right_offset + 1] = 255; // G
                        buffer[right_offset + 2] = 0; // R
                        buffer[right_offset + 3] = 255; // A
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