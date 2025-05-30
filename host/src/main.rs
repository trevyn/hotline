use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use std::time::Duration;

#[cfg(feature = "reload")]
use hot_lib::*;
#[cfg(not(feature = "reload"))]
use lib::*;

#[cfg(feature = "reload")]
#[hot_lib_reloader::hot_module(dylib = "lib", file_watch_debounce = 20, lib_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/debug")
)]
mod hot_lib {
    pub use lib::Box;
    hot_functions_from_file!("lib/src/lib.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
}

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

    let mut boxes = Vec::new();
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
                        let box_x = start_x.min(x);
                        let box_y = start_y.min(y);
                        let box_w = (start_x - x).abs() as u32;
                        let box_h = (start_y - y).abs() as u32;

                        if box_w > 0 && box_h > 0 {
                            boxes.push(create_box(box_x, box_y, box_w, box_h));
                        }
                        drag_start = None;
                    }
                }
                _ => {}
            }
        }

        if let Some((start_x, start_y)) = drag_start {
            let mouse_x = event_pump.mouse_state().x();
            let mouse_y = event_pump.mouse_state().y();

            canvas.set_draw_color(Color::RGBA(255, 255, 255, 128));
            let preview_rect = Rect::new(
                start_x.min(mouse_x),
                start_y.min(mouse_y),
                (start_x - mouse_x).abs() as u32,
                (start_y - mouse_y).abs() as u32,
            );
            let _ = canvas.draw_rect(preview_rect);
        }

        // render boxes to texture
        let mut texture = texture_creator
            .create_texture_streaming(PixelFormatEnum::ARGB8888, 800, 600)
            .map_err(|e| e.to_string())?;

        texture.with_lock(None, |buffer: &mut [u8], _pitch: usize| {
            // clear buffer
            for byte in buffer.iter_mut() {
                *byte = 0;
            }

            // render boxes
            for b in &boxes {
                render_box(b, buffer, 800, 600);
            }
        })?;

        canvas.copy(&texture, None, None)?;
        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
    }

    Ok(())
}
