use hotline::HotlineObject;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;
use sdl2::pixels::{Color, PixelFormatEnum};

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
        gpu_renderer: Option<GPURenderer>,
        gpu_atlases: Vec<AtlasData>,
        gpu_commands: Vec<RenderCommand>,
        fps_counter: Option<TextRenderer>,
        frame_times: std::collections::VecDeque<std::time::Instant>,
        last_fps_update: Option<std::time::Instant>,
        current_fps: f64,
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

            // Create GPU renderer
            self.gpu_renderer = Some(GPURenderer::new());

            // Create window manager
            self.window_manager = Some(WindowManager::new());
            if let Some(ref mut wm) = self.window_manager {
                wm.initialize();

                // Set up GPU rendering
                if let Some(ref mut gpu) = self.gpu_renderer {
                    wm.setup_gpu_rendering(gpu);
                }
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

            // Create FPS counter
            self.fps_counter = Some(TextRenderer::new());
            if let Some(ref mut fps) = self.fps_counter {
                fps.set_x(10.0);
                fps.set_y(10.0);
                fps.set_color((0, 255, 0, 255)); // Green color (BGRA)
                fps.set_text("FPS: 0".to_string());
            }

            // Initialize FPS tracking
            self.frame_times = std::collections::VecDeque::with_capacity(120);
            self.last_fps_update = Some(std::time::Instant::now());
            self.current_fps = 0.0;

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

            let mut canvas = window.into_canvas().present_vsync().build().map_err(|e| e.to_string())?;
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
                // Track frame time
                let now = std::time::Instant::now();
                self.frame_times.push_back(now);

                // Remove old frame times (keep last 2 seconds)
                while let Some(front) = self.frame_times.front() {
                    if now.duration_since(*front).as_secs_f64() > 2.0 {
                        self.frame_times.pop_front();
                    } else {
                        break;
                    }
                }

                // Update FPS every 100ms
                if let Some(last_update) = self.last_fps_update {
                    if now.duration_since(last_update).as_millis() >= 100 {
                        if self.frame_times.len() > 1 {
                            let duration = now.duration_since(*self.frame_times.front().unwrap()).as_secs_f64();
                            self.current_fps = (self.frame_times.len() - 1) as f64 / duration;

                            if let Some(ref mut fps) = self.fps_counter {
                                fps.set_text(format!("FPS: {:.1}", self.current_fps));
                            }
                        }
                        self.last_fps_update = Some(now);
                    }
                }

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
                        Event::MouseWheel { y, .. } => {
                            if let Some(ref mut editor) = self.code_editor {
                                if editor.is_focused() {
                                    editor.scroll_by(-y as f64 * 20.0);
                                }
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

                // GPU render on top
                if let (Some(gpu), Some(wm)) = (&mut self.gpu_renderer, &mut self.window_manager) {
                    wm.render_gpu(gpu);
                }

                // Take GPU renderer out temporarily to avoid borrow issues
                if let Some(mut gpu) = self.gpu_renderer.take() {
                    // Clear previous frame data
                    self.gpu_atlases.clear();
                    self.gpu_commands.clear();

                    // GPU sends render messages to us
                    gpu.render_via(self)?;

                    // Put GPU renderer back
                    self.gpu_renderer = Some(gpu);

                    // Execute the received commands
                    self.execute_gpu_render(&mut canvas)?;
                }

                canvas.copy(&texture, None, None)?;
                canvas.present();
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

                // Render FPS counter
                if let Some(ref mut fps) = self.fps_counter {
                    fps.render(buffer, 800, 600, pitch as i64);
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

                // Render FPS counter
                if let Some(ref mut fps) = self.fps_counter {
                    fps.render(buffer, 800, 600, pitch as i64);
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

        pub fn gpu_receive_atlas(&mut self, atlas: AtlasData) -> Result<(), String> {
            self.gpu_atlases.push(atlas);
            Ok(())
        }

        pub fn gpu_receive_command(&mut self, command: RenderCommand) -> Result<(), String> {
            self.gpu_commands.push(command);
            Ok(())
        }

        fn execute_gpu_render(&mut self, canvas: &mut sdl2::render::Canvas<sdl2::video::Window>) -> Result<(), String> {
            use sdl2::rect::Rect;
            use std::collections::HashMap;

            let texture_creator = canvas.texture_creator();
            let mut textures = HashMap::new();

            // Create textures from received atlases
            for atlas in &self.gpu_atlases {
                let mut texture = match atlas.format {
                    AtlasFormat::GrayscaleAlpha => texture_creator
                        .create_texture_static(PixelFormatEnum::ABGR8888, atlas.width, atlas.height)
                        .map_err(|e| e.to_string())?,
                    AtlasFormat::RGBA => texture_creator
                        .create_texture_static(PixelFormatEnum::RGBA8888, atlas.width, atlas.height)
                        .map_err(|e| e.to_string())?,
                };

                // Convert atlas data to texture format
                let rgba_data = match atlas.format {
                    AtlasFormat::GrayscaleAlpha => {
                        let mut rgba = vec![0u8; (atlas.width * atlas.height * 4) as usize];
                        for i in 0..(atlas.width * atlas.height) as usize {
                            let _gray = atlas.data[i * 2];
                            let alpha = atlas.data[i * 2 + 1];
                            rgba[i * 4] = alpha; // A
                            rgba[i * 4 + 1] = 255; // B
                            rgba[i * 4 + 2] = 255; // G
                            rgba[i * 4 + 3] = 255; // R
                        }
                        rgba
                    }
                    AtlasFormat::RGBA => atlas.data.clone(),
                };

                texture.update(None, &rgba_data, (atlas.width * 4) as usize).map_err(|e| e.to_string())?;
                textures.insert(atlas.id, texture);
            }

            // Execute received render commands
            for command in &self.gpu_commands {
                match command {
                    RenderCommand::Atlas { texture_id, src_x, src_y, src_width, src_height, dest_x, dest_y, color } => {
                        if let Some(texture) = textures.get(texture_id) {
                            let src_rect = Rect::new(*src_x as i32, *src_y as i32, *src_width, *src_height);
                            let dst_rect = Rect::new(*dest_x as i32, *dest_y as i32, *src_width, *src_height);

                            // Apply color modulation
                            canvas.set_draw_color(sdl2::pixels::Color::RGBA(
                                color.2, // R
                                color.1, // G
                                color.0, // B
                                color.3, // A
                            ));

                            canvas.copy(texture, src_rect, dst_rect)?;
                        }
                    }
                }
            }

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
