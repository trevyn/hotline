use sdl3::event::Event;
use sdl3::keyboard::Keycode;
use sdl3::mouse::MouseButton;

use std::time::{Duration, Instant};

pub mod gpu_renderer;

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

// Wrapper to make Starfield work with EventHandler trait
struct StarfieldAdapter {
    starfield: Starfield,
}

impl StarfieldAdapter {
    fn starfield_mut(&mut self) -> &mut Starfield {
        &mut self.starfield
    }
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
    // TODO: Update to use new GPU API
    // fn render_gpu(&mut self, gpu_renderer: &mut GpuRenderer) {
    //     self.chat.generate_commands(gpu_renderer);
    // }
}

impl StarfieldAdapter {
    fn new(starfield: Starfield) -> Self {
        Self { starfield }
    }
}

impl hotline::EventHandler for StarfieldAdapter {
    fn handle_mouse_down(&mut self, x: f64, y: f64) -> bool {
        self.starfield.handle_mouse_down(x, y)
    }

    fn handle_mouse_up(&mut self, x: f64, y: f64) -> bool {
        self.starfield.handle_mouse_up(x, y)
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64) -> bool {
        self.starfield.handle_mouse_move(x, y)
    }

    fn update(&mut self) {
        // Starfield updates itself in generate_commands
    }

    fn render(&mut self, _buffer: &mut [u8], _width: i64, _height: i64, _pitch: i64) {
        // GPU only rendering
    }
}

hotline::object!({
    pub struct Application {
        window_manager: Option<WindowManager>,
        #[serde(skip)]
        event_handlers: Vec<Box<dyn hotline::EventHandler>>,
        #[serde(skip)]
        gpu_renderer: Option<gpu_renderer::GpuRenderer>,
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
        #[serde(skip)]
        last_gpu_print: Option<std::time::Instant>,
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
        starfield: Option<Starfield>,
        chat_interface: Option<ChatInterface>,
        white_pixel_atlas_id: Option<u32>,
    }

    impl Application {
        // Helper to transform mouse coordinates
        fn transform_mouse_coords(&self, x: f32, y: f32, window: &sdl3::video::Window) -> (f64, f64) {
            let (win_w, win_h) = window.size();
            let scale_x = self.width as f64 / win_w as f64;
            let scale_y = self.height as f64 / win_h as f64;
            // Transform to texture coordinates
            let adj_x = x as f64 * scale_x / self.pixel_multiple as f64;
            let adj_y = y as f64 * scale_y / self.pixel_multiple as f64;
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
            // GPU renderer will be initialized when window is available

            // Create a shared white pixel texture for all objects
            if let Some(ref mut gpu) = self.gpu_renderer {
                let white_pixel = vec![255u8, 255, 255, 255]; // RGBA
                let id = gpu.create_texture(&white_pixel, 1, 1, sdl3::gpu::TextureFormat::R8g8b8a8Unorm)?;
                self.white_pixel_atlas_id = Some(id);
            }

            // Create window manager
            self.window_manager = Some(WindowManager::new());
            if let Some(ref mut wm) = self.window_manager {
                wm.initialize();

                // Set up GPU rendering
                if let Some(ref mut gpu) = self.gpu_renderer {
                    // TODO: Update to use new GPU API
                    // wm.setup_gpu_rendering(gpu);
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
                fps.set_color((0, 255, 0, 255)); // Green color (tuple order is B,G,R,A)
                fps.set_text("FPS: 0".to_string());

                // Register GPU atlas for FPS counter
                if let Some(ref mut gpu) = self.gpu_renderer {
                    // TODO: Update to use new GPU API
                    // fps.register_atlas(gpu);
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
            self.last_gpu_print = None;

            // Create game controller display
            self.game_controller = Some(GameController::new());
            if let Some(ref mut gc) = self.game_controller {
                gc.initialize();
                let rect = Rect::new();
                let mut r_ref = rect.clone();
                r_ref.initialize(200.0, 400.0, 200.0, 370.0);
                gc.set_rect(r_ref);

                // Set up GPU rendering
                if let Some(ref mut gpu) = self.gpu_renderer {
                    gc.setup_gpu_rendering(gpu);
                }
            }

            // Create starfield (will be sized to full window in run())
            let mut starfield = Starfield::new();
            starfield.initialize();
            // Don't set rect here - will be set to full window size in run()

            // Set up GPU rendering
            if let Some(ref mut gpu) = self.gpu_renderer {
                starfield.setup_gpu_rendering(gpu);
            }

            // Store a clone for Application's reference
            self.starfield = Some(starfield.clone());

            // Add starfield as event handler
            self.event_handlers.push(Box::new(StarfieldAdapter::new(starfield)));

            Ok(())
        }

        pub fn run(&mut self) -> Result<(), String> {
            // Allow joystick events even when window is not in focus
            sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");

            // Enable GPU driver
            eprintln!("[Application] Setting SDL_RENDER_DRIVER hint to 'gpu'");
            sdl3::hint::set("SDL_RENDER_DRIVER", "gpu");

            let sdl_context = sdl3::init().map_err(|e| e.to_string())?;
            let video_subsystem = sdl_context.video().map_err(|e| e.to_string())?;
            let game_controller_subsystem = sdl_context.gamepad().map_err(|e| e.to_string())?;

            let display = video_subsystem.get_primary_display().map_err(|e| e.to_string())?;
            let usable_bounds = display.get_usable_bounds().map_err(|e| e.to_string())?;
            let win_w = (usable_bounds.width() as f32 * 0.9) as u32;
            let win_h = (usable_bounds.height() as f32 * 0.9) as u32;
            eprintln!("Window size: {}x{}", win_w, win_h);

            let window = video_subsystem
                .window("hotline - direct calls", win_w, win_h)
                .position_centered()
                .high_pixel_density()
                .resizable()
                .build()
                .map_err(|e| e.to_string())?;

            // Initialize GPU renderer with the window
            match gpu_renderer::GpuRenderer::new(&window) {
                Ok(renderer) => {
                    self.gpu_renderer = Some(renderer);
                }
                Err(e) => {
                    return Err(format!("Failed to initialize GPU renderer: {}", e));
                }
            }

            let mut event_pump = sdl_context.event_pump().map_err(|e| e.to_string())?;
            video_subsystem.text_input().start(&window);

            let (dw, dh) = window.size_in_pixels();
            self.width = dw;
            self.height = dh;

            // Set starfield to full window size
            if let Some(ref mut sf) = self.starfield {
                let rect = Rect::new();
                let mut r_ref = rect.clone();
                r_ref.initialize(0.0, 0.0, (dw / self.pixel_multiple) as f64, (dh / self.pixel_multiple) as f64);
                sf.set_rect(r_ref);
            }

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

                // Handle events
                for event in event_pump.poll_iter() {
                    match event {
                        Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                            break 'running;
                        }
                        Event::Window { win_event: sdl3::event::WindowEvent::Resized(_, _), .. }
                        | Event::Window { win_event: sdl3::event::WindowEvent::PixelSizeChanged(_, _), .. } => {
                            let (dw, dh) = window.size_in_pixels();
                            self.width = dw;
                            self.height = dh;

                            // Update starfield to new window size
                            if let Some(ref mut sf) = self.starfield {
                                let rect = Rect::new();
                                let mut r_ref = rect.clone();
                                r_ref.initialize(
                                    0.0,
                                    0.0,
                                    (dw / self.pixel_multiple) as f64,
                                    (dh / self.pixel_multiple) as f64,
                                );
                                sf.set_rect(r_ref);
                            }
                        }
                        Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } => {
                            let (adj_x, adj_y) = self.transform_mouse_coords(x, y, &window);

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
                            let (adj_x, adj_y) = self.transform_mouse_coords(x, y, &window);

                            // TODO: Add right-click support to EventHandler trait if needed

                            if let Some(ref mut wm) = self.window_manager {
                                wm.handle_right_click(adj_x, adj_y);
                            }
                            self.mouse_x = x as f64;
                            self.mouse_y = y as f64;
                        }
                        Event::MouseButtonUp { mouse_btn: MouseButton::Left, x, y, .. } => {
                            let (adj_x, adj_y) = self.transform_mouse_coords(x, y, &window);

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
                            let (adj_x, adj_y) = self.transform_mouse_coords(x, y, &window);

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
                            let (win_w, win_h) = window.size();
                            let scale_x = self.width as f64 / win_w as f64;
                            let scale_y = self.height as f64 / win_h as f64;
                            let adj_x = self.mouse_x * scale_x / self.pixel_multiple as f64;
                            let adj_y = self.mouse_y * scale_y / self.pixel_multiple as f64;

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
                                }
                                Keycode::Minus | Keycode::KpMinus if cmd && self.pixel_multiple > 1 => {
                                    self.pixel_multiple -= 1;
                                    if let Some(ref mut zoom) = self.zoom_display {
                                        zoom.set_text(format!("{}x", self.pixel_multiple));
                                    }
                                    self.zoom_display_until = Some(Instant::now() + Duration::from_secs(1));
                                }
                                Keycode::R => {
                                    // Check if any handler is focused (i.e., editing)
                                    let editing = self.event_handlers.iter().any(|h| h.is_focused());
                                    if !editing {
                                        if shift {
                                            // Shift+R: Randomize starfield parameters
                                            if let Some(ref mut sf) = self.starfield {
                                                sf.randomize_params();
                                            }
                                        } else {
                                            // R: Rotate selected window
                                            if let Some(ref mut wm) = self.window_manager {
                                                wm.rotate_selected(0.1);
                                            }
                                        }
                                    }
                                }
                                Keycode::Tab => {
                                    // Toggle starfield parameter panel
                                    if let Some(ref mut sf) = self.starfield {
                                        sf.toggle_panel();
                                    }
                                }
                                Keycode::M => {
                                    // Toggle movement mode
                                    let editing = self.event_handlers.iter().any(|h| h.is_focused());
                                    if !editing {
                                        if let Some(ref mut sf) = self.starfield {
                                            sf.toggle_movement_mode();
                                        }
                                    }
                                }
                                Keycode::Minus | Keycode::KpMinus => {
                                    // Decrease starfield acceleration
                                    if let Some(ref mut sf) = self.starfield {
                                        let current = sf.acceleration_multiplier();
                                        let new_val = (current - 0.5).max(0.1);
                                        sf.set_acceleration_multiplier(new_val);
                                        eprintln!("Starfield acceleration: {:.1}x", new_val);
                                    }
                                }
                                Keycode::Plus | Keycode::Equals | Keycode::KpPlus => {
                                    // Increase starfield acceleration
                                    if let Some(ref mut sf) = self.starfield {
                                        let current = sf.acceleration_multiplier();
                                        let new_val = (current + 0.5).min(20.0);
                                        sf.set_acceleration_multiplier(new_val);
                                        eprintln!("Starfield acceleration: {:.1}x", new_val);
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
                                let (win_w, win_h) = window.size();
                                let scale_x = self.width as f64 / win_w as f64;
                                let scale_y = self.height as f64 / win_h as f64;
                                let adj_x = self.mouse_x * scale_x / self.pixel_multiple as f64;
                                let adj_y = self.mouse_y * scale_y / self.pixel_multiple as f64;

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

                // Update starfield with controller input and sync with event handler
                if let (Some(sf), Some(gc)) = (&mut self.starfield, &self.game_controller) {
                    let (lx, ly, rx, ry) = gc.axis_values();
                    let (lt, rt) = gc.trigger_values();
                    sf.update_controller(lx, ly, rx, ry, lt, rt);

                    // Update the starfield in the event handler to keep them in sync
                    // Find the StarfieldAdapter in event_handlers and update it
                    // (This is a bit hacky but necessary due to the split architecture)
                }

                // Begin GPU frame
                if let Some(gpu) = &mut self.gpu_renderer {
                    // Begin frame
                    gpu.begin_frame();
                } else {
                    // No GPU renderer
                }

                // Render objects using new GPU API
                if let Some(gpu) = &mut self.gpu_renderer {
                    // Render WindowManager rects
                    if let Some(wm) = &mut self.window_manager {
                        // Directly render rects from WindowManager
                        for i in 0..wm.get_rects_count() {
                            if let Some(rect) = wm.get_rect_at(i) {
                                let (x, y, w, h) = rect.bounds();

                                // Generate color based on position and time (matching Rect's render method)
                                let t = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_millis();
                                let r = (x as u32 % 255) as f32 / 255.0;
                                let g = (y as u32 % 128) as f32 / 255.0;
                                let b = (t / 6 % 255) as f32 / 255.0;
                                let a = 1.0;

                                gpu.add_solid_rect(x as f32, y as f32, w as f32, h as f32, [r, g, b, a]);
                            }
                        }
                    }

                    // Render GameController
                    if let Some(gc) = &mut self.game_controller {
                        gc.render_gpu(gpu);
                    }

                    // Render Starfield
                    if let Some(sf) = &mut self.starfield {
                        sf.render_gpu(gpu);
                    }

                    // Render ColorWheel
                    if let Some(cw) = &mut self.color_wheel {
                        // TODO: Update ColorWheel to use new GPU API
                    }

                    // Render checkboxes
                    if let Some(cb) = &mut self.autonomy_checkbox {
                        // TODO: Update Checkbox to use new GPU API
                    }
                    if let Some(cb) = &mut self.render_time_checkbox {
                        // TODO: Update Checkbox to use new GPU API
                    }

                    // Render FPS counter
                    if let Some(fps) = &mut self.fps_counter {
                        fps.render_gpu(gpu);
                    }

                    // Render code editor through event handler
                    for handler in &mut self.event_handlers {
                        // TODO: Update event handlers to support GPU rendering
                    }
                }

                // Render using SDL3 GPU API
                if let Some(gpu) = &mut self.gpu_renderer {
                    // Render frame
                    gpu.render_frame(&window)?;
                } else {
                    eprintln!("[Application] ERROR: gpu_renderer is None during render!");
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
