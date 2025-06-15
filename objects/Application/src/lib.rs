use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::mouse::MouseButton;
use sdl3::pixels::{Color, PixelFormat};
use sdl3::render::BlendMode;
use std::convert::TryFrom;

use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use png::{BitDepth, ColorType, Encoder};
#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::BufWriter;

// Wrapper to make CodeEditor work with EventHandler trait
struct CodeEditorAdapter {
    editor: CodeEditor,
}

impl CodeEditorAdapter {
    fn new(editor: CodeEditor) -> Self {
        Self { editor }
    }
}

impl hotline::EventHandler for CodeEditorAdapter {
    fn handle_mouse_down(&mut self, x: f64, y: f64) -> bool {
        self.editor.handle_mouse_down(x, y)
    }

    fn handle_mouse_up(&mut self, _x: f64, _y: f64) -> bool {
        self.editor.handle_mouse_up();
        false
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64) -> bool {
        self.editor.handle_mouse_move(x, y);
        false
    }

    fn handle_mouse_wheel(&mut self, _x: f64, _y: f64, delta: f64) -> bool {
        if self.editor.is_focused() {
            self.editor.add_scroll_velocity(-delta * 20.0);
            true
        } else {
            false
        }
    }

    fn handle_text_input(&mut self, text: &str) -> bool {
        if self.editor.is_focused() {
            for ch in text.chars() {
                self.editor.insert_char(ch);
            }
            true
        } else {
            false
        }
    }

    fn handle_key_down(&mut self, keycode: i32, shift: bool) -> bool {
        if !self.editor.is_focused() {
            return false;
        }

        // Handle common keycodes directly
        match keycode {
            8 => {
                // Backspace
                self.editor.backspace();
                true
            }
            13 => {
                // Return
                self.editor.insert_newline();
                true
            }
            1073741904 => {
                // Left arrow
                self.editor.move_cursor_left(shift);
                true
            }
            1073741903 => {
                // Right arrow
                self.editor.move_cursor_right(shift);
                true
            }
            1073741906 => {
                // Up arrow
                self.editor.move_cursor_up(shift);
                true
            }
            1073741905 => {
                // Down arrow
                self.editor.move_cursor_down(shift);
                true
            }
            _ => false,
        }
    }

    fn is_focused(&self) -> bool {
        // Can't call is_focused on editor because it requires mutable borrow
        // TODO: Fix this by making is_focused() immutable in CodeEditor
        false
    }

    fn update(&mut self) {
        self.editor.update_scroll();
    }

    fn render(&mut self, buffer: &mut [u8], width: i64, height: i64, pitch: i64) {
        self.editor.render(buffer, width, height, pitch);
    }
}

// Wrapper to make ChatInterface work with EventHandler trait
struct ChatInterfaceAdapter {
    chat: ChatInterface,
}

impl ChatInterfaceAdapter {
    fn new(chat: ChatInterface) -> Self {
        Self { chat }
    }
}

impl hotline::EventHandler for ChatInterfaceAdapter {
    fn handle_mouse_down(&mut self, x: f64, y: f64) -> bool {
        self.chat.handle_mouse_down(x, y)
    }

    fn handle_mouse_up(&mut self, _x: f64, _y: f64) -> bool {
        self.chat.handle_mouse_up();
        false
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64) -> bool {
        self.chat.handle_mouse_move(x, y);
        false
    }

    fn handle_mouse_wheel(&mut self, x: f64, y: f64, delta: f64) -> bool {
        self.chat.handle_mouse_wheel(x, y, delta);
        true
    }

    fn handle_text_input(&mut self, text: &str) -> bool {
        for ch in text.chars() {
            self.chat.insert_char(ch);
        }
        true
    }

    fn handle_key_down(&mut self, keycode: i32, _shift: bool) -> bool {
        // Handle common keycodes directly
        match keycode {
            8 => {
                // Backspace
                self.chat.backspace();
                true
            }
            13 => {
                // Return
                self.chat.insert_char('\n');
                true
            }
            1073741904 => {
                // Left arrow
                self.chat.move_cursor_left();
                true
            }
            1073741903 => {
                // Right arrow
                self.chat.move_cursor_right();
                true
            }
            _ => false,
        }
    }

    fn update(&mut self) {
        self.chat.update_scroll();
    }

    fn render(&mut self, buffer: &mut [u8], width: i64, height: i64, pitch: i64) {
        // Re-enable CPU rendering
        self.chat.render(buffer, width, height, pitch);
    }
}

impl ChatInterfaceAdapter {
    fn render_gpu(&mut self, gpu_renderer: &mut GPURenderer) {
        self.chat.generate_commands(gpu_renderer);
    }
}

hotline::object!({
    pub struct Application {
        window_manager: Option<WindowManager>,
        #[serde(skip)]
        event_handlers: Vec<Box<dyn hotline::EventHandler>>,
        gpu_renderer: Option<GPURenderer>,
        #[serde(skip)]
        gpu_atlases: Vec<AtlasData>,
        #[serde(skip)]
        gpu_commands: Vec<RenderCommand>,
        #[serde(skip)]
        gpu_texture_cache: std::collections::HashMap<u32, Vec<u8>>,
        fps_counter: Option<TextRenderer>,
        autonomy_checkbox: Option<Checkbox>,
        render_time_checkbox: Option<Checkbox>,
        color_wheel: Option<ColorWheel>,
        anthropic_client: Option<AnthropicClient>,
        #[serde(skip)]
        frame_times: std::collections::VecDeque<std::time::Instant>,
        #[serde(skip)]
        last_fps_update: Option<std::time::Instant>,
        current_fps: f64,
        mouse_x: f64,
        mouse_y: f64,
        #[default(2)]
        pixel_multiple: u32,
        width: u32,
        height: u32,
        zoom_display: Option<TextRenderer>,
        #[serde(skip)]
        zoom_display_until: Option<std::time::Instant>,
        game_controller: Option<GameController>,
        chat_interface: Option<ChatInterface>,
        white_pixel_atlas_id: Option<u32>,
    }

    impl Application {
        // Helper to transform mouse coordinates
        fn transform_mouse_coords(
            &self,
            x: f32,
            y: f32,
            canvas: &sdl3::render::Canvas<sdl3::video::Window>,
        ) -> (f64, f64) {
            let (win_w, win_h) = canvas.window().size();
            let scale_x = self.width as f64 / win_w as f64;
            let scale_y = self.height as f64 / win_h as f64;
            // With SDL render scale, don't divide by pixel_multiple
            let adj_x = x as f64 * scale_x;
            let adj_y = y as f64 * scale_y;
            (adj_x, adj_y)
        }

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

            // Create a shared white pixel atlas for all objects
            if let Some(ref mut gpu) = self.gpu_renderer {
                let white_pixel = vec![255u8, 255, 255, 255]; // RGBA
                let id = gpu.register_atlas(white_pixel, 1, 1, AtlasFormat::RGBA);
                self.white_pixel_atlas_id = Some(id);
            }

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
            let mut editor = CodeEditor::new();
            let _ = editor.open("objects/Rect/src/lib.rs");

            // Create rect for editor
            let editor_rect = Rect::new();
            let mut editor_rect_ref = editor_rect.clone();
            editor_rect_ref.initialize(400.0, 50.0, 380.0, 500.0);

            if let Some(ref mut wm) = self.window_manager {
                wm.add_rect(editor_rect.clone());
            }
            editor.set_rect(editor_rect);

            // Add editor as event handler
            self.event_handlers.push(Box::new(CodeEditorAdapter::new(editor)));

            // Create color wheel
            self.color_wheel = Some(ColorWheel::new());
            if let Some(ref mut wheel) = self.color_wheel {
                let rect = Rect::new();
                let mut r_ref = rect.clone();
                r_ref.initialize(50.0, 400.0, 120.0, 120.0);
                wheel.set_rect(rect);
            }

            // Create chat interface with AnthropicClient
            let mut chat = ChatInterface::new();
            chat.initialize();

            // Pass shared white atlas to chat
            if let Some(atlas_id) = self.white_pixel_atlas_id {
                chat.set_shared_white_atlas(atlas_id);
            }

            // Set bounds for chat interface
            let chat_rect = Rect::new();
            let mut chat_rect_ref = chat_rect.clone();
            chat_rect_ref.initialize(800.0, 50.0, 500.0, 700.0);
            chat.set_rect(chat_rect);

            // Create and connect AnthropicClient
            let mut client = AnthropicClient::new();
            client.initialize();
            client.set_response_target(&chat);

            // Connect client to chat
            chat.set_anthropic_client(&client);

            // Store anthropic client for later use
            self.anthropic_client = Some(client);

            // Set up GPU rendering for chat - DISABLED
            // if let Some(ref mut gpu) = self.gpu_renderer {
            //     chat.register_atlases(gpu);
            // }

            // Store chat interface
            self.chat_interface = Some(chat.clone());

            // Add chat as event handler
            self.event_handlers.push(Box::new(ChatInterfaceAdapter::new(chat)));

            // Create autonomy checkbox
            self.autonomy_checkbox = Some(Checkbox::new());
            if let Some(ref mut cb) = self.autonomy_checkbox {
                let rect = Rect::new();
                let mut r_ref = rect.clone();
                r_ref.initialize(20.0, 60.0, 20.0, 20.0);
                cb.set_rect(rect);
                cb.set_label("Autonomy".to_string());
            }

            // Create render time checkbox
            self.render_time_checkbox = Some(Checkbox::new());
            if let Some(ref mut cb) = self.render_time_checkbox {
                let rect = Rect::new();
                let mut r_ref = rect.clone();
                r_ref.initialize(20.0, 90.0, 20.0, 20.0);
                cb.set_rect(rect);
                cb.set_label("Render Times".to_string());
            }

            // Create FPS counter
            self.fps_counter = Some(TextRenderer::new());
            if let Some(ref mut fps) = self.fps_counter {
                fps.initialize();
                fps.set_x(10.0);
                fps.set_y(10.0);
                fps.set_color((255, 0, 255, 0)); // Green color (ABGR)
                fps.set_text("FPS: 0".to_string());

                // Register GPU atlas for FPS counter
                if let Some(ref mut gpu) = self.gpu_renderer {
                    fps.register_atlas(gpu);
                }
            }

            // Zoom display text
            self.zoom_display = Some(TextRenderer::new());
            if let Some(ref mut zoom) = self.zoom_display {
                zoom.set_x(10.0);
                zoom.set_y(30.0);
                zoom.set_color((255, 255, 255, 255)); // White in ABGR
                zoom.set_text("2x".to_string());
            }

            // Initialize FPS tracking
            self.frame_times = std::collections::VecDeque::with_capacity(120);
            self.last_fps_update = Some(std::time::Instant::now());
            self.current_fps = 0.0;

            // Create game controller display
            self.game_controller = Some(GameController::new());
            if let Some(ref mut gc) = self.game_controller {
                gc.initialize();
                let rect = Rect::new();
                let mut r_ref = rect.clone();
                r_ref.initialize(200.0, 400.0, 200.0, 370.0);
                gc.set_rect(rect);

                // Register GPU atlases once during initialization
                if let Some(ref mut gpu) = self.gpu_renderer {
                    gc.register_atlases(gpu);
                }
            }

            Ok(())
        }

        pub fn run(&mut self) -> Result<(), String> {
            let sdl_context = sdl3::init().map_err(|e| e.to_string())?;
            let video_subsystem = sdl_context.video().map_err(|e| e.to_string())?;
            let game_controller_subsystem = sdl_context.gamepad().map_err(|e| e.to_string())?;

            let display = video_subsystem.get_primary_display().map_err(|e| e.to_string())?;
            let usable_bounds = display.get_usable_bounds().map_err(|e| e.to_string())?;
            let win_w = (usable_bounds.width() as f32 * 0.9) as u32;
            let win_h = (usable_bounds.height() as f32 * 0.9) as u32;

            let window = video_subsystem
                .window("hotline - direct calls", win_w, win_h)
                .position_centered()
                .high_pixel_density()
                .resizable()
                .build()
                .map_err(|e| e.to_string())?;

            let mut canvas = sdl3::render::create_renderer(window, None).map_err(|e| e.to_string())?;

            // Query the native texture format from SDL3
            use sdl3::sys::everything::SDL_PROP_RENDERER_TEXTURE_FORMATS_POINTER;
            use sdl3::sys::properties::{SDL_GetPointerProperty, SDL_PropertiesID};
            use sdl3::sys::render::SDL_GetRendererProperties;

            let renderer_ptr = canvas.raw();
            let props_id: SDL_PropertiesID = unsafe { SDL_GetRendererProperties(renderer_ptr) };

            let formats_ptr = unsafe {
                SDL_GetPointerProperty(props_id, SDL_PROP_RENDERER_TEXTURE_FORMATS_POINTER, std::ptr::null_mut())
            };

            let mut native_format_u32: u32 = 0x16362004; // SDL_PIXELFORMAT_ARGB8888

            if !formats_ptr.is_null() {
                let formats = unsafe { formats_ptr as *const u32 };
                eprintln!("Native texture formats:");

                // Read formats until we hit 0 (SDL_PIXELFORMAT_UNKNOWN)
                let mut i = 0;
                loop {
                    let format = unsafe { *formats.offset(i) };
                    if format == 0 {
                        break;
                    }

                    // Convert format code to name
                    let format_name = match format {
                        0x15151002 => "SDL_PIXELFORMAT_RGB565",
                        0x16161804 => "SDL_PIXELFORMAT_ARGB8888",
                        0x16261804 => "SDL_PIXELFORMAT_RGBA8888",
                        0x16362004 => "SDL_PIXELFORMAT_ABGR8888",
                        0x16462004 => "SDL_PIXELFORMAT_BGRA8888",
                        _ => "Unknown format",
                    };

                    eprintln!("  Format {}: {} (0x{:08x})", i, format_name, format);
                    i += 1;
                }

                // Use the first format as our native format
                if i > 0 {
                    native_format_u32 = unsafe { *formats };
                    eprintln!("Using native format: 0x{:08x}", native_format_u32);
                }
            } else {
                eprintln!("Could not query native texture formats, using ARGB8888");
            }

            let texture_creator = canvas.texture_creator();
            let mut event_pump = sdl_context.event_pump().map_err(|e| e.to_string())?;
            video_subsystem.text_input().start(canvas.window());

            let (dw, dh) = canvas.output_size().map_err(|e| e.to_string())?;
            self.width = dw;
            self.height = dh;

            // Create texture at full resolution
            let mut texture = texture_creator
                .create_texture_streaming(
                    PixelFormat::try_from(native_format_u32 as i64).unwrap(),
                    self.width,
                    self.height,
                )
                .map_err(|e| e.to_string())?;
            texture.set_scale_mode(sdl3::render::ScaleMode::Nearest);

            // Set initial render scale
            canvas.set_scale(self.pixel_multiple as f32, self.pixel_multiple as f32).map_err(|e| e.to_string())?;

            // Check for connected game controllers
            let controllers = game_controller_subsystem.gamepads().map_err(|e| e.to_string())?;
            eprintln!("Found {} game controllers", controllers.len());

            // Open the first available controller
            let mut _controller = None;
            if !controllers.is_empty() {
                let controller_id = controllers[0];
                match game_controller_subsystem.open(controller_id) {
                    Ok(controller) => {
                        eprintln!("Opened game controller {}: {:?}", controller_id, controller.name());
                        if let Some(ref mut gc) = self.game_controller {
                            gc.set_connected(true, Some(controller_id));
                        }
                        _controller = Some(controller);
                    }
                    Err(e) => {
                        eprintln!("Failed to open game controller {}: {}", controller_id, e);
                    }
                }
            }

            #[cfg(target_os = "linux")]
            {
                self.run_linux_test(&mut texture)?;
                return Ok(());
            }

            #[cfg_attr(target_os = "linux", allow(unreachable_code))]
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

                canvas.set_draw_color(Color::RGB(30, 30, 30));
                canvas.clear();

                // Handle events
                for event in event_pump.poll_iter() {
                    match event {
                        Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                            break 'running;
                        }
                        Event::Window { win_event: sdl3::event::WindowEvent::Resized(_, _), .. }
                        | Event::Window { win_event: sdl3::event::WindowEvent::PixelSizeChanged(_, _), .. } => {
                            let (dw, dh) = canvas.window().size_in_pixels();
                            self.width = dw;
                            self.height = dh;
                            texture = texture_creator
                                .create_texture_streaming(
                                    PixelFormat::try_from(sdl3::sys::everything::SDL_PIXELFORMAT_ARGB8888).unwrap(),
                                    self.width / self.pixel_multiple,
                                    self.height / self.pixel_multiple,
                                )
                                .map_err(|e| e.to_string())?;
                            texture.set_scale_mode(sdl3::render::ScaleMode::Nearest);
                        }
                        Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } => {
                            let (adj_x, adj_y) = self.transform_mouse_coords(x, y, &canvas);

                            let mut consumed = false;
                            // Dispatch to event handlers in order
                            for handler in &mut self.event_handlers {
                                if handler.handle_mouse_down(adj_x, adj_y) {
                                    consumed = true;
                                    break;
                                }
                            }

                            if let Some(ref mut wm) = self.window_manager {
                                if !consumed {
                                    wm.handle_mouse_down(adj_x, adj_y);
                                    let hits = wm.inspect_click(adj_x, adj_y);
                                    if hits.is_empty() {
                                        if wm.is_resizing() {
                                            if let Some(items) = wm.selected_info_lines() {
                                                wm.open_inspector(items);
                                            }
                                        } else {
                                            wm.close_inspector();
                                        }
                                    } else {
                                        wm.open_inspector(hits);
                                    }
                                } else {
                                    wm.close_inspector();
                                }
                            }

                            self.mouse_x = x as f64;
                            self.mouse_y = y as f64;
                            if let Some(ref mut wheel) = self.color_wheel {
                                if let Some(_color) = wheel.handle_mouse_down(adj_x, adj_y) {
                                    // TODO: Need a way to notify editor of color change
                                }
                            }
                            if let Some(ref mut cb) = self.autonomy_checkbox {
                                cb.handle_mouse_down(adj_x, adj_y);
                            }
                            if let Some(ref mut cb) = self.render_time_checkbox {
                                cb.handle_mouse_down(adj_x, adj_y);
                            }
                        }
                        Event::MouseButtonDown { mouse_btn: MouseButton::Right, x, y, .. } => {
                            let (adj_x, adj_y) = self.transform_mouse_coords(x, y, &canvas);

                            // TODO: Add right-click support to EventHandler trait if needed

                            if let Some(ref mut wm) = self.window_manager {
                                wm.handle_right_click(adj_x, adj_y);
                            }
                            self.mouse_x = x as f64;
                            self.mouse_y = y as f64;
                        }
                        Event::MouseButtonUp { mouse_btn: MouseButton::Left, x, y, .. } => {
                            let (adj_x, adj_y) = self.transform_mouse_coords(x, y, &canvas);

                            if let Some(ref mut wm) = self.window_manager {
                                wm.handle_mouse_up(adj_x, adj_y);
                            }

                            // Dispatch to all event handlers
                            for handler in &mut self.event_handlers {
                                handler.handle_mouse_up(adj_x, adj_y);
                            }

                            if let Some(ref mut wheel) = self.color_wheel {
                                wheel.handle_mouse_up();
                            }
                            self.mouse_x = x as f64;
                            self.mouse_y = y as f64;
                        }
                        Event::MouseMotion { x, y, .. } => {
                            let (adj_x, adj_y) = self.transform_mouse_coords(x, y, &canvas);

                            if let Some(ref mut wm) = self.window_manager {
                                wm.handle_mouse_motion(adj_x, adj_y);
                            }

                            // Dispatch to all event handlers
                            for handler in &mut self.event_handlers {
                                handler.handle_mouse_move(adj_x, adj_y);
                            }

                            if let Some(ref mut wheel) = self.color_wheel {
                                if let Some(_color) = wheel.handle_mouse_move(adj_x, adj_y) {
                                    // TODO: Need a way to notify editor of color change
                                }
                            }
                            self.mouse_x = x as f64;
                            self.mouse_y = y as f64;
                        }
                        Event::MouseWheel { y, .. } => {
                            let (win_w, win_h) = canvas.window().size();
                            let scale_x = self.width as f64 / win_w as f64;
                            let scale_y = self.height as f64 / win_h as f64;
                            let adj_x = self.mouse_x * scale_x;
                            let adj_y = self.mouse_y * scale_y;

                            // Dispatch to event handlers in order
                            for handler in &mut self.event_handlers {
                                if handler.handle_mouse_wheel(adj_x, adj_y, y as f64) {
                                    break;
                                }
                            }
                        }
                        Event::TextInput { text, .. } => {
                            // Dispatch to event handlers in order
                            for handler in &mut self.event_handlers {
                                if handler.handle_text_input(&text) {
                                    break;
                                }
                            }
                        }
                        Event::KeyDown { keycode: Some(kc), keymod, .. } => {
                            let shift = keymod.contains(sdl3::keyboard::Mod::LSHIFTMOD)
                                || keymod.contains(sdl3::keyboard::Mod::RSHIFTMOD);

                            // Check for cmd/ctrl modifier
                            #[cfg(target_os = "macos")]
                            let cmd = keymod.contains(sdl3::keyboard::Mod::LGUIMOD)
                                || keymod.contains(sdl3::keyboard::Mod::RGUIMOD);
                            #[cfg(not(target_os = "macos"))]
                            let cmd = keymod.contains(sdl3::keyboard::Mod::LCTRLMOD)
                                || keymod.contains(sdl3::keyboard::Mod::RCTRLMOD);

                            // Handle specific keys first
                            match kc {
                                Keycode::Equals | Keycode::KpPlus if cmd => {
                                    self.pixel_multiple += 1;
                                    if let Some(ref mut zoom) = self.zoom_display {
                                        zoom.set_text(format!("{}x", self.pixel_multiple));
                                    }
                                    self.zoom_display_until = Some(Instant::now() + Duration::from_secs(1));
                                    // Use SDL_SetRenderScale instead of recreating texture
                                    canvas
                                        .set_scale(self.pixel_multiple as f32, self.pixel_multiple as f32)
                                        .map_err(|e| e.to_string())?;
                                }
                                Keycode::Minus | Keycode::KpMinus if cmd && self.pixel_multiple > 1 => {
                                    self.pixel_multiple -= 1;
                                    if let Some(ref mut zoom) = self.zoom_display {
                                        zoom.set_text(format!("{}x", self.pixel_multiple));
                                    }
                                    self.zoom_display_until = Some(Instant::now() + Duration::from_secs(1));
                                    // Use SDL_SetRenderScale instead of recreating texture
                                    canvas
                                        .set_scale(self.pixel_multiple as f32, self.pixel_multiple as f32)
                                        .map_err(|e| e.to_string())?;
                                }
                                Keycode::R => {
                                    // Check if any handler is focused (i.e., editing)
                                    let editing = self.event_handlers.iter().any(|h| h.is_focused());
                                    if !editing {
                                        if let Some(ref mut wm) = self.window_manager {
                                            wm.rotate_selected(0.1);
                                        }
                                    }
                                }
                                Keycode::S if cmd => {
                                    // TODO: Add save support to EventHandler trait if needed
                                }
                                _ => {
                                    // Convert keycode to i32 for EventHandler trait
                                    let keycode_i32 = kc as i32;

                                    // Dispatch to event handlers in order
                                    for handler in &mut self.event_handlers {
                                        if handler.handle_key_down(keycode_i32, shift) {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        Event::DropFile { filename, .. } => {
                            if filename.to_lowercase().ends_with(".png") {
                                let (win_w, win_h) = canvas.window().size();
                                let scale_x = self.width as f64 / win_w as f64;
                                let scale_y = self.height as f64 / win_h as f64;
                                let adj_x = self.mouse_x * scale_x;
                                let adj_y = self.mouse_y * scale_y;

                                if let Some(ref mut wm) = self.window_manager {
                                    if let Some(registry) = wm.get_registry() {
                                        ::hotline::set_library_registry(registry);
                                    }
                                    let mut img = Image::new();
                                    img.initialize(adj_x, adj_y);
                                    img.load_png(&filename)?;
                                    wm.add_image(img);
                                }
                            }
                        }
                        Event::ControllerAxisMotion { which, axis, value, .. } => {
                            if let Some(ref mut gc) = self.game_controller {
                                // SDL3 axes: 0=LeftX, 1=LeftY, 2=RightX, 3=RightY, 4=LeftTrigger, 5=RightTrigger
                                let axis_idx = axis as u8;
                                let normalized_value = if axis_idx >= 4 {
                                    // Triggers: 0 to 32767 -> 0.0 to 1.0
                                    value.max(0) as f32 / 32767.0
                                } else {
                                    // Sticks: -32768 to 32767 -> -1.0 to 1.0
                                    value as f32 / 32768.0
                                };
                                gc.update_axis(axis_idx, normalized_value);
                            }
                        }
                        Event::ControllerButtonDown { which, button, .. } => {
                            if let Some(ref mut gc) = self.game_controller {
                                gc.update_button(button as u8, true);
                            }
                        }
                        Event::ControllerButtonUp { which, button, .. } => {
                            if let Some(ref mut gc) = self.game_controller {
                                gc.update_button(button as u8, false);
                            }
                        }
                        Event::ControllerDeviceAdded { which, .. } => {
                            eprintln!("Game controller {} connected", which);
                            // Store the controller id to open it later if needed
                            if _controller.is_none() {
                                match game_controller_subsystem.open(which) {
                                    Ok(controller) => {
                                        eprintln!("Opened game controller {}: {:?}", which, controller.name());
                                        if let Some(ref mut gc) = self.game_controller {
                                            gc.set_connected(true, Some(which));
                                        }
                                        _controller = Some(controller);
                                    }
                                    Err(e) => {
                                        eprintln!("Failed to open game controller {}: {}", which, e);
                                    }
                                }
                            }
                        }
                        Event::ControllerDeviceRemoved { which, .. } => {
                            eprintln!("Game controller {} disconnected", which);
                            if let Some(ref mut gc) = self.game_controller {
                                gc.set_connected(false, None);
                            }
                        }
                        // Also handle raw joystick events as fallback
                        Event::JoyAxisMotion { which, axis_idx, value, .. } => {
                            if let Some(ref mut gc) = self.game_controller {
                                let normalized_value = if axis_idx >= 4 {
                                    // Triggers: 0 to 32767 -> 0.0 to 1.0
                                    value.max(0) as f32 / 32767.0
                                } else {
                                    // Sticks: -32768 to 32767 -> -1.0 to 1.0
                                    value as f32 / 32768.0
                                };
                                gc.update_axis(axis_idx, normalized_value);
                            }
                        }
                        Event::JoyButtonDown { which, button_idx, .. } => {
                            if let Some(ref mut gc) = self.game_controller {
                                gc.update_button(button_idx, true);
                            }
                        }
                        Event::JoyButtonUp { which, button_idx, .. } => {
                            if let Some(ref mut gc) = self.game_controller {
                                gc.update_button(button_idx, false);
                            }
                        }
                        Event::JoyDeviceAdded { which, .. } => {
                            eprintln!("Joystick {} connected", which);
                            if let Some(ref mut gc) = self.game_controller {
                                gc.set_connected(true, Some(which));
                            }
                        }
                        Event::JoyDeviceRemoved { which, .. } => {
                            eprintln!("Joystick {} disconnected", which);
                            if let Some(ref mut gc) = self.game_controller {
                                gc.set_connected(false, None);
                            }
                        }
                        _ => {}
                    }
                }

                if let (Some(wm), Some(cb)) = (&mut self.window_manager, &mut self.autonomy_checkbox) {
                    if cb.checked() {
                        wm.update_autonomy(self.mouse_x, self.mouse_y);
                    }
                }
                if let (Some(wm), Some(cb)) = (&mut self.window_manager, &mut self.render_time_checkbox) {
                    wm.set_show_render_times(cb.checked());
                }
                // Update all event handlers
                for handler in &mut self.event_handlers {
                    handler.update();
                }

                // Skip CPU render frame entirely
                // self.render_frame(&mut texture)?;

                // GPU render on top
                if let (Some(gpu), Some(wm)) = (&mut self.gpu_renderer, &mut self.window_manager) {
                    wm.render_gpu(gpu);
                }

                // GPU render game controller
                if let (Some(gpu), Some(gc)) = (&mut self.gpu_renderer, &mut self.game_controller) {
                    // Only generate commands, don't re-register atlases every frame
                    gc.generate_commands(gpu);
                }

                // GPU render FPS counter
                if let (Some(gpu), Some(fps)) = (&mut self.gpu_renderer, &mut self.fps_counter) {
                    fps.generate_commands(gpu);
                }

                // GPU render chat interface - DISABLED to isolate segfault
                // if let (Some(gpu), Some(chat)) = (&mut self.gpu_renderer, &mut self.chat_interface) {
                //     chat.generate_commands(gpu);
                // }

                // Process GPU rendering
                if let Some(gpu) = &mut self.gpu_renderer {
                    // Clear commands AND atlases from previous frame
                    self.gpu_commands.clear();
                    self.gpu_atlases.clear();

                    // Only collect new atlases we haven't seen before
                    let gpu_atlases = gpu.get_atlases();
                    for atlas in gpu_atlases {
                        // Check if we already have this atlas
                        if !self.gpu_atlases.iter().any(|a| a.id == atlas.id) {
                            self.gpu_atlases.push(atlas);
                        }
                    }

                    // Collect commands for this frame
                    for command in gpu.get_commands() {
                        self.gpu_commands.push(command.clone());
                    }

                    // Execute the received commands
                    self.execute_gpu_render(&mut canvas, native_format_u32)?;
                }

                // Skip texture copy - GPU renders directly to canvas
                // canvas.copy(&texture, None, None).map_err(|e| e.to_string())?;
                canvas.present();
            }

            Ok(())
        }

        fn render_frame(&mut self, texture: &mut sdl3::render::Texture) -> Result<(), String> {
            let query = texture.query();
            let _bw = query.width as i64;
            let _bh = query.height as i64;

            // Skip texture lock/clear when using GPU-only rendering
            // texture
            //     .with_lock(None, |buffer: &mut [u8], pitch: usize| {
            //         // Clear buffer
            //         for pixel in buffer.chunks_exact_mut(4) {
            //             pixel[0] = 30; // B
            //             pixel[1] = 30; // G
            //             pixel[2] = 30; // R
            //             pixel[3] = 255; // A
            //         }

            //         // NO CPU RENDERING - GPU ONLY
            //         let _ = (buffer, bw, bh, pitch);
            //     })
            //     .map_err(|e| e.to_string())?;
            Ok(())
        }

        #[cfg(target_os = "linux")]
        fn run_linux_test(&mut self, texture: &mut sdl3::render::Texture) -> Result<(), String> {
            println!("[linux] creating test rects");

            if let Some(ref mut wm) = self.window_manager {
                wm.handle_mouse_down(50.0, 50.0);
                wm.handle_mouse_up(250.0, 150.0);
                wm.handle_mouse_down(300.0, 200.0);
                wm.handle_mouse_up(450.0, 350.0);
            }

            println!("[linux] rendering");
            let q = texture.query();
            let bw = q.width as i64;
            let bh = q.height as i64;
            let mut png_data = vec![0u8; (bw * bh * 4) as usize];
            texture
                .with_lock(None, |buffer: &mut [u8], pitch: usize| {
                    // Clear buffer
                    for pixel in buffer.chunks_exact_mut(4) {
                        pixel[0] = 30; // B
                        pixel[1] = 30; // G
                        pixel[2] = 30; // R
                        pixel[3] = 255; // A
                    }

                    // Render window manager
                    if let Some(ref mut wm) = self.window_manager {
                        wm.render(buffer, bw, bh, pitch as i64);
                    }

                    // Render all event handlers
                    for handler in &mut self.event_handlers {
                        handler.render(buffer, bw, bh, pitch as i64);
                    }

                    if let Some(ref mut wheel) = self.color_wheel {
                        wheel.render(buffer, bw, bh, pitch as i64);
                    }

                    if let Some(ref mut cb) = self.autonomy_checkbox {
                        cb.render(buffer, 800, 600, pitch as i64);
                    }

                    // Render FPS counter
                    if let Some(ref mut fps) = self.fps_counter {
                        fps.render(buffer, bw, bh, pitch as i64);
                    }

                    for y in 0..bh {
                        for x in 0..bw {
                            let src = (y * pitch as i64 + x * 4) as usize;
                            let dst = (y * bw + x) as usize * 4;
                            png_data[dst] = buffer[src + 2];
                            png_data[dst + 1] = buffer[src + 1];
                            png_data[dst + 2] = buffer[src];
                            png_data[dst + 3] = buffer[src + 3];
                        }
                    }
                })
                .map_err(|e| e.to_string())?;

            println!("[linux] saving test_output.png");
            save_png("test_output.png", bw as u32, bh as u32, &png_data)?;
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

        fn execute_gpu_render(
            &mut self,
            canvas: &mut sdl3::render::Canvas<sdl3::video::Window>,
            native_format_u32: u32,
        ) -> Result<(), String> {
            use sdl3::rect::Rect;
            use std::collections::HashMap;

            if self.frame_times.len() % 60 == 0 {
                eprintln!(
                    "GPU render: {} atlases, {} commands, FPS: {:.1}",
                    self.gpu_atlases.len(),
                    self.gpu_commands.len(),
                    self.current_fps
                );
            }

            let texture_creator = canvas.texture_creator();
            let mut textures = HashMap::new();

            // Create textures for all atlases
            for atlas in &self.gpu_atlases {
                let mut texture = match atlas.format {
                    AtlasFormat::GrayscaleAlpha => texture_creator
                        .create_texture_static(
                            PixelFormat::try_from(native_format_u32 as i64).unwrap(),
                            atlas.width,
                            atlas.height,
                        )
                        .map_err(|e| e.to_string())?,
                    AtlasFormat::RGBA => texture_creator
                        .create_texture_static(
                            PixelFormat::try_from(native_format_u32 as i64).unwrap(),
                            atlas.width,
                            atlas.height,
                        )
                        .map_err(|e| e.to_string())?,
                };

                // Convert atlas data to texture format
                let rgba_data = match atlas.format {
                    AtlasFormat::GrayscaleAlpha => {
                        let mut rgba = vec![0u8; (atlas.width * atlas.height * 4) as usize];
                        for i in 0..(atlas.width * atlas.height) as usize {
                            // For font atlases: gray channel contains the glyph coverage
                            let coverage = atlas.data[i * 2]; // This is the opacity of the glyph
                            let _alpha = atlas.data[i * 2 + 1]; // Usually 255 for fonts

                            // For ABGR8888 format - use coverage in all channels
                            rgba[i * 4] = coverage; // A
                            rgba[i * 4 + 1] = coverage; // B
                            rgba[i * 4 + 2] = coverage; // G
                            rgba[i * 4 + 3] = coverage; // R
                        }
                        rgba
                    }
                    AtlasFormat::RGBA => {
                        // Convert from RGBA to ABGR
                        let mut abgr = vec![0u8; atlas.data.len()];
                        for i in 0..(atlas.width * atlas.height) as usize {
                            let r = atlas.data[i * 4];
                            let g = atlas.data[i * 4 + 1];
                            let b = atlas.data[i * 4 + 2];
                            let a = atlas.data[i * 4 + 3];
                            abgr[i * 4] = a; // A
                            abgr[i * 4 + 1] = b; // B
                            abgr[i * 4 + 2] = g; // G
                            abgr[i * 4 + 3] = r; // R
                        }
                        abgr
                    }
                };

                texture.update(None, &rgba_data, (atlas.width * 4) as usize).map_err(|e| e.to_string())?;

                // Enable blending for text atlases
                if matches!(atlas.format, AtlasFormat::GrayscaleAlpha) {
                    texture.set_blend_mode(BlendMode::Blend);
                }

                textures.insert(atlas.id, texture);
            }

            // Execute received render commands
            for command in &self.gpu_commands {
                match command {
                    RenderCommand::Atlas { texture_id, src_x, src_y, src_width, src_height, dest_x, dest_y, color } => {
                        if let Some(texture) = textures.get_mut(texture_id) {
                            let src_rect = Rect::new(*src_x as i32, *src_y as i32, *src_width, *src_height);
                            let dst_rect = Rect::new(*dest_x as i32, *dest_y as i32, *src_width, *src_height);

                            // Set texture color modulation
                            // SDL expects RGB order for set_color_mod regardless of texture format
                            // The color tuple is (A, B, G, R) - ABGR order in our data
                            texture.set_color_mod(color.3, color.2, color.1); // R, G, B
                            texture.set_alpha_mod(color.0); // A

                            canvas.copy(texture, src_rect, dst_rect).map_err(|e| e.to_string())?;
                        }
                    }
                    RenderCommand::Rect { texture_id, dest_x, dest_y, dest_width, dest_height, rotation: _, color } => {
                        // eprintln!(
                        //     "  Rect command: id={} pos=({},{}) size=({},{})",
                        //     texture_id, dest_x, dest_y, dest_width, dest_height
                        // );
                        // Skip invalid rectangles
                        if *dest_width <= 0.0 || *dest_height <= 0.0 {
                            continue;
                        }

                        if let Some(texture) = textures.get_mut(texture_id) {
                            let dst_rect =
                                Rect::new(*dest_x as i32, *dest_y as i32, *dest_width as u32, *dest_height as u32);

                            // Set texture color modulation
                            // SDL expects RGB order for set_color_mod regardless of texture format
                            // The color tuple is (A, B, G, R) - ABGR order in our data
                            texture.set_color_mod(color.3, color.2, color.1); // R, G, B
                            texture.set_alpha_mod(color.0); // A

                            // eprintln!("  Drawing rect at {:?}", dst_rect);
                            canvas.copy(texture, None, dst_rect).map_err(|e| e.to_string())?;
                        } else {
                            // eprintln!("  WARNING: Texture {} not found!", texture_id);
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
