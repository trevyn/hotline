use hotline::HotlineObject;

hotline::object!({
    #[derive(Clone, Copy, PartialEq, Default)]
    enum ResizeDir {
        #[default]
        None,
        Left,
        Right,
        Top,
        Bottom,
        TopLeft,
        TopRight,
        BottomLeft,
        BottomRight,
    }

    #[derive(Default)]
    pub struct WindowManager {
        rects: Vec<Rect>,
        polygons: Vec<RegularPolygon>,
        selected: Option<Rect>,
        highlight_lens: Option<HighlightLens>, // HighlightLens for selected rect
        text_renderer: Option<TextRenderer>,   // TextRenderer for displaying text
        context_menu: Option<ContextMenu>,
        dragging: bool,
        drag_offset_x: f64,
        drag_offset_y: f64,
        drag_start: Option<(f64, f64)>,
        resizing: bool,
        resize_dir: ResizeDir,
        resize_start: Option<(f64, f64)>,
        resize_orig: Option<(f64, f64, f64, f64)>,
    }

    impl WindowManager {
        pub fn initialize(&mut self) {
            // Initialize text renderer using the registry stored on this object
            if let Some(registry) = self.get_registry() {
                // Set the registry in thread-local storage for TextRenderer::new()
                ::hotline::set_library_registry(registry);

                // Now create text renderer
                let text_renderer = TextRenderer::new()
                    .with_text("Hello, Hotline!".to_string())
                    .with_x(20.0)
                    .with_y(20.0)
                    .with_color((0, 255, 255, 255)); // Yellow text in BGRA format
                self.text_renderer = Some(text_renderer);
            } else {
                panic!("WindowManager registry not initialized");
            }
        }

        pub fn add_rect(&mut self, rect: Rect) {
            self.rects.push(rect);
        }

        pub fn clear_selection(&mut self) {
            self.selected = None;
            self.highlight_lens = None;
            self.dragging = false;
            self.resizing = false;
            self.resize_dir = ResizeDir::None;
            self.resize_start = None;
            self.resize_orig = None;
        }

        pub fn set_drag_offset(&mut self, x: f64, y: f64) {
            self.drag_offset_x = x;
            self.drag_offset_y = y;
        }

        pub fn start_dragging(&mut self, rect: Rect) {
            self.selected = Some(rect);
            self.dragging = true;
        }

        pub fn stop_dragging(&mut self) {
            self.dragging = false;
        }

        pub fn get_selected_handle(&mut self) -> Option<Rect> {
            self.selected.clone()
        }

        pub fn get_rects_count(&mut self) -> i64 {
            self.rects.len() as i64
        }

        pub fn get_rect_at(&mut self, index: i64) -> Option<Rect> {
            if index >= 0 && (index as usize) < self.rects.len() {
                Some(self.rects[index as usize].clone())
            } else {
                None
            }
        }

        pub fn is_dragging(&mut self) -> bool {
            self.dragging
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) {
            if let Some(ref mut menu) = self.context_menu {
                if let Some(selection) = menu.handle_mouse_down(x, y) {
                    match selection.as_str() {
                        "Rect" => {
                            let mut r = Rect::new();
                            r.initialize(x, y, 100.0, 100.0);
                            self.rects.push(r);
                        }
                        "RegularPolygon" => {
                            let mut p = RegularPolygon::new();
                            p.initialize(x, y, 40.0, 5);
                            self.polygons.push(p);
                        }
                        _ => {}
                    }
                }
                self.context_menu = None;
                return;
            }

            // First check for hits
            let mut hit_index = None;
            let mut hit_position = (0.0, 0.0);
            let mut resize_dir = ResizeDir::None;
            let mut resize_bounds = None;

            for (i, rect_handle) in self.rects.iter_mut().enumerate().rev() {
                // Check if this rect contains the point
                let (rx, ry, rw, rh) = rect_handle.bounds();
                let margin = 5.0;
                let inside = rect_handle.contains_point(x, y);
                let near_left = (x - rx).abs() <= margin && y >= ry - margin && y <= ry + rh + margin;
                let near_right = (x - (rx + rw)).abs() <= margin && y >= ry - margin && y <= ry + rh + margin;
                let near_top = (y - ry).abs() <= margin && x >= rx - margin && x <= rx + rw + margin;
                let near_bottom = (y - (ry + rh)).abs() <= margin && x >= rx - margin && x <= rx + rw + margin;

                if near_left && near_top {
                    resize_dir = ResizeDir::TopLeft;
                } else if near_right && near_top {
                    resize_dir = ResizeDir::TopRight;
                } else if near_left && near_bottom {
                    resize_dir = ResizeDir::BottomLeft;
                } else if near_right && near_bottom {
                    resize_dir = ResizeDir::BottomRight;
                } else if near_left {
                    resize_dir = ResizeDir::Left;
                } else if near_right {
                    resize_dir = ResizeDir::Right;
                } else if near_top {
                    resize_dir = ResizeDir::Top;
                } else if near_bottom {
                    resize_dir = ResizeDir::Bottom;
                }

                if resize_dir != ResizeDir::None || inside {
                    hit_index = Some(i);
                    hit_position = rect_handle.position();
                    resize_bounds = Some(rect_handle.bounds());
                    break;
                }
            }

            // Clear previous selection
            self.clear_selection();

            if let Some(index) = hit_index {
                // Found a hit - select it
                let mut rect_clone = self.rects[index].clone();
                self.selected = Some(rect_clone.clone());

                // Create HighlightLens for selected rect
                self.highlight_lens = Some(HighlightLens::new().with_target(&rect_clone).with_show_handles(true));

                if resize_dir != ResizeDir::None {
                    self.resizing = true;
                    self.resize_dir = resize_dir;
                    self.resize_start = Some((x, y));
                    self.resize_orig = resize_bounds;
                    if let Some(ref mut lens) = self.highlight_lens {
                        lens.set_highlight_color((255, 255, 0, 255));
                    }
                } else {
                    // Calculate drag offset from click position to rect position
                    self.drag_offset_x = hit_position.0 - x;
                    self.drag_offset_y = hit_position.1 - y;
                    self.dragging = true;
                }
            } else {
                // No hit - start rect creation
                self.drag_start = Some((x, y));
            }
        }

        pub fn handle_mouse_up(&mut self, x: f64, y: f64) {
            if self.context_menu.is_some() {
                return;
            }
            else if self.resizing {
                self.resizing = false;
                self.resize_dir = ResizeDir::None;
                self.resize_start = None;
                self.resize_orig = None;
                if let Some(ref mut lens) = self.highlight_lens {
                    lens.set_highlight_color((0, 255, 0, 255));
                }
            } else if self.dragging {
                self.stop_dragging();
            } else if let Some((start_x, start_y)) = self.drag_start {
                // Create a new rect directly
                let width = (x - start_x).abs();
                let height = (y - start_y).abs();
                let rect_x = start_x.min(x);
                let rect_y = start_y.min(y);

                // Create new rect
                let mut rect_handle = Rect::new();
                rect_handle.initialize(rect_x, rect_y, width, height);
                self.rects.push(rect_handle);

                self.drag_start = None;
            }
        }

        pub fn handle_mouse_motion(&mut self, x: f64, y: f64) {
            if self.context_menu.is_some() {
                return;
            }
            if self.dragging {
                if let Some(ref mut selected_handle) = self.selected {
                    // Move the selected rect to follow the mouse
                    let new_x = x + self.drag_offset_x;
                    let new_y = y + self.drag_offset_y;

                    // Get current position
                    let (current_x, current_y) = selected_handle.position();

                    // Calculate delta movement
                    let dx = new_x - current_x;
                    let dy = new_y - current_y;

                    // Move the rect
                    selected_handle.move_by(dx, dy);
                }
            } else if self.resizing {
                if let (
                    Some(ref mut selected_handle),
                    Some((start_x, start_y)),
                    Some((orig_x, orig_y, orig_w, orig_h)),
                ) = (self.selected.as_mut(), self.resize_start, self.resize_orig)
                {
                    let dx = x - start_x;
                    let dy = y - start_y;
                    let mut new_x = orig_x;
                    let mut new_y = orig_y;
                    let mut new_w = orig_w;
                    let mut new_h = orig_h;

                    match self.resize_dir {
                        ResizeDir::Left => {
                            new_x = orig_x + dx;
                            new_w = orig_w - dx;
                        }
                        ResizeDir::Right => {
                            new_w = orig_w + dx;
                        }
                        ResizeDir::Top => {
                            new_y = orig_y + dy;
                            new_h = orig_h - dy;
                        }
                        ResizeDir::Bottom => {
                            new_h = orig_h + dy;
                        }
                        ResizeDir::TopLeft => {
                            new_x = orig_x + dx;
                            new_w = orig_w - dx;
                            new_y = orig_y + dy;
                            new_h = orig_h - dy;
                        }
                        ResizeDir::TopRight => {
                            new_w = orig_w + dx;
                            new_y = orig_y + dy;
                            new_h = orig_h - dy;
                        }
                        ResizeDir::BottomLeft => {
                            new_x = orig_x + dx;
                            new_w = orig_w - dx;
                            new_h = orig_h + dy;
                        }
                        ResizeDir::BottomRight => {
                            new_w = orig_w + dx;
                            new_h = orig_h + dy;
                        }
                        ResizeDir::None => {}
                    }

                    if new_w < 1.0 {
                        new_w = 1.0;
                    }
                    if new_h < 1.0 {
                        new_h = 1.0;
                    }

                    selected_handle.resize(new_x, new_y, new_w, new_h);
                }
            }
        }

        pub fn handle_right_click(&mut self, x: f64, y: f64) {
            let mut menu = self.context_menu.take().unwrap_or_else(ContextMenu::new);
            menu.open(x, y);
            self.context_menu = Some(menu);
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            // Render all rects
            for rect_handle in &mut self.rects {
                rect_handle.render(buffer, buffer_width, buffer_height, pitch);
            }

            // Render polygons
            for poly in &mut self.polygons {
                poly.render(buffer, buffer_width, buffer_height, pitch);
            }

            // Render the highlight lens if we have one (this will render the selected rect with highlight)
            if let Some(ref mut hl_handle) = self.highlight_lens {
                hl_handle.render(buffer, buffer_width, buffer_height, pitch);
            }

            // Render text
            if let Some(ref mut text_renderer) = self.text_renderer {
                text_renderer.render(buffer, buffer_width, buffer_height, pitch);
            }

            // Render context menu if visible
            if let Some(ref mut menu) = self.context_menu {
                menu.render(buffer, buffer_width, buffer_height, pitch);
            }
        }

        pub fn setup_gpu_rendering(&mut self, gpu_renderer: &mut GPURenderer) {
            // Register text atlas if we have one
            if let Some(ref mut text_renderer) = self.text_renderer {
                if text_renderer.has_atlas() {
                    let atlas_id = gpu_renderer.register_atlas(
                        text_renderer.atlas_data(),
                        text_renderer.atlas_dimensions().0,
                        text_renderer.atlas_dimensions().1,
                        AtlasFormat::GrayscaleAlpha,
                    );
                    text_renderer.set_atlas_id(atlas_id);
                }
            }
        }

        pub fn render_gpu(&mut self, gpu_renderer: &mut GPURenderer) {
            gpu_renderer.clear_commands();

            // Generate render commands from text renderer
            if let Some(ref mut text_renderer) = self.text_renderer {
                text_renderer.generate_commands(gpu_renderer);
            }
        }
    }
});
