use runtime::{Runtime, m};
use hotline::{ObjectHandle, Value};
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
                    drag_start = Some((x, y));
                }
                Event::MouseButtonUp {
                    mouse_btn: MouseButton::Left,
                    x,
                    y,
                    ..
                } => {
                    if let Some((start_x, start_y)) = drag_start {
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
        })?;
        
        // Copy texture to canvas
        canvas.copy(&texture, None, None)?;

        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}