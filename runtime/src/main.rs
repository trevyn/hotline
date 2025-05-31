use runtime::{Runtime, m};
use hotline::ObjectHandle;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::{Color, PixelFormatEnum};
use std::time::Duration;

fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window("hotline", 800, 600)
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
    let texture_creator = canvas.texture_creator();
    let mut event_pump = sdl_context.event_pump()?;

    let mut runtime = Runtime::new();
    
    // Load rect object dynamically
    #[cfg(target_os = "macos")]
    let lib_path = "target/release/librect.dylib";
    #[cfg(target_os = "linux")]
    let lib_path = "target/release/librect.so";
    #[cfg(target_os = "windows")]
    let lib_path = "target/release/rect.dll";
    
    runtime.hot_reload(lib_path)
        .expect("Failed to load rect library");
    
    // Or use static linking
    #[cfg(not(feature = "reload"))]
    {
        // Static linking would require rect to be a dependency
        // For now, just use dynamic loading
    }

    let mut rects: Vec<ObjectHandle> = Vec::new();
    let mut drag_start = None;
    let mut selected: Option<ObjectHandle> = None;
    let mut dragging = false;
    let mut drag_offset = (0.0, 0.0);

    'running: loop {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                Event::MouseButtonDown {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    // Check if clicking on existing rect
                    selected = runtime.hit_test(x as f64, y as f64);
                    if let Some(handle) = selected {
                        // Start dragging the rect
                        dragging = true;
                        // Calculate offset from rect origin to click point
                        if let Some(bounds) = runtime.get_bounds(handle) {
                            drag_offset = (x as f64 - bounds.x, y as f64 - bounds.y);
                        }
                    } else {
                        // Start creating new rect
                        drag_start = Some((x, y));
                    }
                }
                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    if dragging {
                        // Stop dragging
                        dragging = false;
                    } else if let Some((start_x, start_y)) = drag_start {
                        // Create new rect
                        let box_x = start_x.min(x) as f64;
                        let box_y = start_y.min(y) as f64;
                        let box_w = (start_x - x).abs() as f64;
                        let box_h = (start_y - y).abs() as f64;

                        if box_w > 0.0 && box_h > 0.0 {
                            if let Some(rect) = runtime.create("Rect") {
                                // Use the macro
                                m![runtime, rect, initWithX:box_x y:box_y width:box_w height:box_h];
                                rects.push(rect);
                            }
                        }
                        drag_start = None;
                    }
                }
                Event::MouseMotion { x, y, .. } => {
                    if dragging {
                        if let Some(handle) = selected {
                            // Update rect position
                            let new_x = x as f64 - drag_offset.0;
                            let new_y = y as f64 - drag_offset.1;
                            if let Some(bounds) = runtime.get_bounds(handle) {
                                let dx = new_x - bounds.x;
                                let dy = new_y - bounds.y;
                                m![runtime, handle, moveBy:dx y:dy];
                            }
                        }
                    }
                }
                // Hot reload on R key
                Event::KeyDown {
                    keycode: Some(Keycode::R),
                    ..
                } => {
                    println!("Reloading rect library...");
                    if let Err(e) = runtime.hot_reload(lib_path) {
                        eprintln!("Failed to reload: {}", e);
                    }
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
            for rect_handle in &rects {
                runtime.render_object(*rect_handle, buffer, 800, 600, pitch as i64);
            }
            
            // Highlight selected rect with a border
            if let Some(sel_handle) = selected {
                if let Some(bounds) = runtime.get_bounds(sel_handle) {
                    // Draw selection border
                    let x_start = (bounds.x as i32).max(0) as u32;
                    let y_start = (bounds.y as i32).max(0) as u32;
                    let x_end = ((bounds.x + bounds.width) as i32).min(800) as u32;
                    let y_end = ((bounds.y + bounds.height) as i32).min(600) as u32;
                    
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
            }
        })?;
        
        // Copy texture to canvas
        canvas.copy(&texture, None, None)?;

        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}