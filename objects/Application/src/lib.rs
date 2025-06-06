use hotline::HotlineObject;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::{Color, PixelFormatEnum};
use std::time::Duration;

#[cfg(target_os = "linux")]
use png::{BitDepth, ColorType, Encoder};
#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::BufWriter;

hotline::object!({
    #[derive(Default)]
    pub struct Application {
        window_manager: Option<WindowManager>,
        code_editor: Option<CodeEditor>,
    }

    impl Application {
        pub fn initialize(&mut self) -> Result<(), String> {
            // Set up thread-local registry for proxy object creation
            // The runtime should have already loaded all libraries
            if let Some(registry) = self.get_registry() {
                hotline::set_library_registry(registry);
            } else {
                return Err("Application registry not available during initialize".into());
            }

            // Create window manager
            self.window_manager = Some(WindowManager::new());
            if let Some(ref mut wm) = self.window_manager {
                wm.initialize();
            }

            // Create code editor
            self.code_editor = Some(CodeEditor::new());
            if let Some(ref mut editor) = self.code_editor {
                let _ = editor.open("objects/Rect/src/lib.rs");

                // Create rect for editor
                let editor_rect = Rect::new();
                let mut editor_rect_ref = editor_rect.clone();
                editor_rect_ref.initialize(400.0, 50.0, 380.0, 500.0);

                if let Some(ref mut wm) = self.window_manager {
                    wm.add_rect(editor_rect.clone());
                }
                editor.set_rect(editor_rect);
            }

            Ok(())
        }

        pub fn run(&mut self) -> Result<(), String> {
            let sdl_context = sdl2::init()?;
            let video_subsystem = sdl_context.video()?;

            let window = video_subsystem
                .window("hotline - direct calls", 800, 600)
                .position_centered()
                .allow_highdpi()
                .build()
                .map_err(|e| e.to_string())?;

            let mut canvas = window.into_canvas().build().map_err(|e| e.to_string())?;
            let texture_creator = canvas.texture_creator();
            let mut event_pump = sdl_context.event_pump()?;
            video_subsystem.text_input().start();

            // Create texture
            let mut texture = texture_creator
                .create_texture_streaming(PixelFormatEnum::ARGB8888, 800, 600)
                .map_err(|e| e.to_string())?;

            #[cfg(target_os = "linux")]
            {
                self.run_linux_test(&mut texture)?;
                return Ok(());
            }

            'running: loop {
                canvas.set_draw_color(Color::RGB(0, 0, 0));
                canvas.clear();

                // Handle events
                for event in event_pump.poll_iter() {
                    match event {
                        Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                            break 'running;
                        }
                        Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } => {
                            if let Some(ref mut wm) = self.window_manager {
                                wm.handle_mouse_down(x as f64, y as f64);
                            }
                            if let Some(ref mut editor) = self.code_editor {
                                editor.handle_mouse_down(x as f64, y as f64);
                            }
                        }
                        Event::MouseButtonUp { mouse_btn: MouseButton::Left, x, y, .. } => {
                            if let Some(ref mut wm) = self.window_manager {
                                wm.handle_mouse_up(x as f64, y as f64);
                            }
                        }
                        Event::MouseMotion { x, y, .. } => {
                            if let Some(ref mut wm) = self.window_manager {
                                wm.handle_mouse_motion(x as f64, y as f64);
                            }
                        }
                        Event::TextInput { text, .. } => {
                            if let Some(ref mut editor) = self.code_editor {
                                if editor.is_focused() {
                                    for ch in text.chars() {
                                        editor.insert_char(ch);
                                    }
                                }
                            }
                        }
                        Event::KeyDown { keycode: Some(Keycode::Backspace), .. } => {
                            if let Some(ref mut editor) = self.code_editor {
                                if editor.is_focused() {
                                    editor.backspace();
                                }
                            }
                        }
                        Event::KeyDown { keycode: Some(Keycode::S), keymod, .. } => {
                            // Check for Cmd+S (Mac) or Ctrl+S (others)
                            #[cfg(target_os = "macos")]
                            let save_key = keymod.contains(sdl2::keyboard::Mod::LGUIMOD)
                                || keymod.contains(sdl2::keyboard::Mod::RGUIMOD);
                            #[cfg(not(target_os = "macos"))]
                            let save_key = keymod.contains(sdl2::keyboard::Mod::LCTRLMOD)
                                || keymod.contains(sdl2::keyboard::Mod::RCTRLMOD);

                            if save_key {
                                if let Some(ref mut editor) = self.code_editor {
                                    if editor.is_focused() {
                                        let _ = editor.save();
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }

                // Render
                self.render_frame(&mut texture)?;

                canvas.copy(&texture, None, None)?;
                canvas.present();
                ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
            }

            Ok(())
        }

        fn render_frame(&mut self, texture: &mut sdl2::render::Texture) -> Result<(), String> {
            texture.with_lock(None, |buffer: &mut [u8], pitch: usize| {
                // Clear buffer
                for pixel in buffer.chunks_exact_mut(4) {
                    pixel[0] = 30; // B
                    pixel[1] = 30; // G
                    pixel[2] = 30; // R
                    pixel[3] = 255; // A
                }

                // Render window manager
                if let Some(ref mut wm) = self.window_manager {
                    wm.render(buffer, 800, 600, pitch as i64);
                }

                // Render code editor
                if let Some(ref mut editor) = self.code_editor {
                    editor.render(buffer, 800, 600, pitch as i64);
                }
            })?;
            Ok(())
        }

        #[cfg(target_os = "linux")]
        fn run_linux_test(&mut self, texture: &mut sdl2::render::Texture) -> Result<(), String> {
            println!("[linux] creating test rects");

            if let Some(ref mut wm) = self.window_manager {
                wm.handle_mouse_down(50.0, 50.0);
                wm.handle_mouse_up(250.0, 150.0);
                wm.handle_mouse_down(300.0, 200.0);
                wm.handle_mouse_up(450.0, 350.0);
            }

            println!("[linux] rendering");
            let mut png_data = vec![0u8; 800 * 600 * 4];
            texture.with_lock(None, |buffer: &mut [u8], pitch: usize| {
                // Clear buffer
                for pixel in buffer.chunks_exact_mut(4) {
                    pixel[0] = 30; // B
                    pixel[1] = 30; // G
                    pixel[2] = 30; // R
                    pixel[3] = 255; // A
                }

                // Render window manager
                if let Some(ref mut wm) = self.window_manager {
                    wm.render(buffer, 800, 600, pitch as i64);
                }

                // Render code editor
                if let Some(ref mut editor) = self.code_editor {
                    editor.render(buffer, 800, 600, pitch as i64);
                }

                for y in 0..600 {
                    for x in 0..800 {
                        let src = y * pitch + x * 4;
                        let dst = (y * 800 + x) * 4;
                        png_data[dst] = buffer[src + 2];
                        png_data[dst + 1] = buffer[src + 1];
                        png_data[dst + 2] = buffer[src];
                        png_data[dst + 3] = buffer[src + 3];
                    }
                }
            })?;

            println!("[linux] saving test_output.png");
            save_png("test_output.png", 800, 600, &png_data)?;
            println!("[linux] image saved");
            Ok(())
        }
    }
});

#[cfg(target_os = "linux")]
fn save_png(path: &str, width: u32, height: u32, data: &[u8]) -> Result<(), String> {
    let file = File::create(path).map_err(|e| e.to_string())?;
    let w = BufWriter::new(file);
    let mut encoder = Encoder::new(w, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(data).map_err(|e| e.to_string())
}
