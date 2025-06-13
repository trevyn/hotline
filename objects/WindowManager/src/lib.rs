#[derive(Clone, Copy, PartialEq)]
pub enum SelectedObject {
    Rect(usize),
    Polygon(usize),
}

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
        rect_movers: Vec<RectMover>,
        polygons: Vec<RegularPolygon>,
        images: Vec<Image>,
        selected: Option<SelectedObject>,
        highlight_lens: Option<HighlightLens>, // HighlightLens for selected rect
        text_renderer: Option<TextRenderer>,   // TextRenderer for displaying text
        context_menu: Option<ContextMenu>,
        polygon_menu: Option<PolygonMenu>,
        click_inspector: Option<ClickInspector>,
        show_render_times: bool,
        rect_time_labels: Vec<TextRenderer>,
        polygon_time_labels: Vec<TextRenderer>,
        dragging: bool,
        drag_offset_x: f64,
        drag_offset_y: f64,
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

                // Create click inspector
                self.click_inspector = Some(ClickInspector::new());
                self.polygon_menu = Some(PolygonMenu::new());
            } else {
                panic!("WindowManager registry not initialized");
            }
        }

        pub fn add_rect(&mut self, rect: Rect) {
            let mut mover = RectMover::new();
            mover.set_target(rect.clone());
            self.rect_movers.push(mover);
            self.rects.push(rect);
        }

        pub fn add_image(&mut self, image: Image) {
            self.images.push(image);
        }

        pub fn inspect_click(&mut self, x: f64, y: f64) -> Vec<String> {
            let mut hits = Vec::new();
            for rect in &mut self.rects {
                if rect.contains_point(x, y) {
                    hits.extend(rect.info_lines());
                }
            }
            for poly in &mut self.polygons {
                if poly.contains_point(x, y) {
                    hits.extend(poly.info_lines());
                }
            }
            hits
        }

        pub fn open_inspector(&mut self, items: Vec<String>) {
            if let Some(ref mut inspector) = self.click_inspector {
                inspector.open(items);
            }
        }

        pub fn close_inspector(&mut self) {
            if let Some(ref mut inspector) = self.click_inspector {
                inspector.close();
            }
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

        pub fn start_dragging(&mut self, selection: SelectedObject) {
            self.selected = Some(selection);
            self.dragging = true;
        }

        pub fn stop_dragging(&mut self) {
            self.dragging = false;
        }

        fn update_highlight(&mut self) {
            if let (Some(sel), Some(ref mut hl)) = (self.selected, self.highlight_lens.as_mut()) {
                let bounds = match sel {
                    SelectedObject::Rect(i) => self.rects[i].bounds(),
                    SelectedObject::Polygon(i) => self.polygons[i].bounds(),
                };
                let mut r = Rect::new();
                r.initialize(bounds.0, bounds.1, bounds.2, bounds.3);
                hl.set_target(&r);
            }
        }

        fn update_inspector(&mut self) {
            if let (Some(ref mut inspector), Some(sel)) = (self.click_inspector.as_mut(), self.selected) {
                if inspector.is_visible() {
                    let items = match sel {
                        SelectedObject::Rect(i) => self.rects[i].info_lines(),
                        SelectedObject::Polygon(i) => self.polygons[i].info_lines(),
                    };
                    inspector.update_items(items);
                }
            }
        }

        pub fn get_selected_handle(&mut self) -> Option<SelectedObject> {
            self.selected
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

        pub fn is_resizing(&self) -> bool {
            self.resizing
        }

        pub fn selected_info_lines(&mut self) -> Option<Vec<String>> {
            match self.selected {
                Some(SelectedObject::Rect(i)) => Some(self.rects[i].info_lines()),
                Some(SelectedObject::Polygon(i)) => Some(self.polygons[i].info_lines()),
                None => None,
            }
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) {
            if let Some(ref mut inspector) = self.click_inspector {
                if inspector.handle_mouse_down(x, y) {
                    return;
                }
            }
            if let Some(ref mut pm) = self.polygon_menu {
                if pm.is_visible() {
                    if let Some(sides) = pm.handle_mouse_down(x, y) {
                        let mut p = RegularPolygon::new();
                        p.initialize(x, y, 40.0, sides);
                        self.polygons.push(p);
                    }
                    self.context_menu = None;
                    return;
                }
            }

            if let Some(ref mut menu) = self.context_menu {
                if let Some(selection) = menu.handle_mouse_down(x, y) {
                    match selection.as_str() {
                        "Rect" => {
                            let mut r = Rect::new();
                            r.initialize(x, y, 100.0, 100.0);
                            self.add_rect(r);
                        }
                        "RegularPolygon" => {
                            if let Some(ref mut pm) = self.polygon_menu {
                                pm.open(x + 100.0, y);
                            }
                        }
                        _ => {}
                    }
                }
                let hide_menu = if let Some(pm) = self.polygon_menu.as_mut() { !pm.is_visible() } else { true };
                if hide_menu {
                    self.context_menu = None;
                }
                return;
            }

            // First check for hits
            let mut hit: Option<(SelectedObject, (f64, f64), (f64, f64, f64, f64))> = None;
            let mut resize_dir = ResizeDir::None;

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
                    hit = Some((SelectedObject::Rect(i), rect_handle.position(), rect_handle.bounds()));
                    break;
                }
            }

            if hit.is_none() {
                for (i, poly) in self.polygons.iter_mut().enumerate().rev() {
                    let (rx, ry, rw, rh) = poly.bounds();
                    let margin = 5.0;
                    let inside = poly.contains_point(x, y);
                    let near_left = (x - rx).abs() <= margin && y >= ry - margin && y <= ry + rh + margin;
                    let near_right = (x - (rx + rw)).abs() <= margin && y >= ry - margin && y <= ry + rh + margin;
                    let near_top = (y - ry).abs() <= margin && x >= rx - margin && x <= rx + rw + margin;
                    let near_bottom = (y - (ry + rh)).abs() <= margin && x >= rx - margin && x <= rx + rw + margin;

                    resize_dir = ResizeDir::None;
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
                        hit = Some((SelectedObject::Polygon(i), poly.position(), poly.bounds()));
                        break;
                    }
                }
            }

            // Clear previous selection
            self.clear_selection();

            if let Some((sel, pos, bounds)) = hit {
                let mut rect_clone = Rect::new();
                rect_clone.initialize(bounds.0, bounds.1, bounds.2, bounds.3);
                self.selected = Some(sel);
                self.highlight_lens = Some(HighlightLens::new().with_target(&rect_clone).with_show_handles(true));

                if resize_dir != ResizeDir::None {
                    self.resizing = true;
                    self.resize_dir = resize_dir;
                    self.resize_start = Some((x, y));
                    self.resize_orig = Some(bounds);
                    if let Some(ref mut lens) = self.highlight_lens {
                        lens.set_highlight_color((255, 255, 0, 255));
                    }
                } else {
                    self.drag_offset_x = pos.0 - x;
                    self.drag_offset_y = pos.1 - y;
                    self.dragging = true;
                }
            }
        }

        pub fn handle_mouse_up(&mut self, x: f64, y: f64) {
            if let Some(ref mut inspector) = self.click_inspector {
                let was_dragging = inspector.is_dragging();
                inspector.handle_mouse_up();
                if was_dragging {
                    return;
                }
            }
            if self.context_menu.is_some() {
                return;
            } else if self.resizing {
                self.resizing = false;
                self.resize_dir = ResizeDir::None;
                self.resize_start = None;
                self.resize_orig = None;
                if let Some(ref mut lens) = self.highlight_lens {
                    lens.set_highlight_color((0, 255, 0, 255));
                }
            } else if self.dragging {
                self.stop_dragging();
            }
        }

        pub fn handle_mouse_motion(&mut self, x: f64, y: f64) {
            if let Some(ref mut inspector) = self.click_inspector {
                inspector.handle_mouse_move(x, y);
                if inspector.is_dragging() {
                    return;
                }
            }
            if let Some(ref mut pm) = self.polygon_menu {
                if pm.is_visible() {
                    pm.handle_mouse_move(x, y);
                    return;
                }
            }
            if self.context_menu.is_some() {
                return;
            }
            if self.dragging {
                if let Some(sel) = self.selected {
                    let new_x = x + self.drag_offset_x;
                    let new_y = y + self.drag_offset_y;
                    match sel {
                        SelectedObject::Rect(i) => {
                            let (cx, cy) = self.rects[i].position();
                            let dx = new_x - cx;
                            let dy = new_y - cy;
                            self.rects[i].move_by(dx, dy);
                        }
                        SelectedObject::Polygon(i) => {
                            let (cx, cy) = self.polygons[i].position();
                            let dx = new_x - cx;
                            let dy = new_y - cy;
                            self.polygons[i].move_by(dx, dy);
                        }
                    }
                    self.update_highlight();
                    self.update_inspector();
                }
            } else if self.resizing {
                if let (Some(sel), Some((start_x, start_y)), Some((orig_x, orig_y, orig_w, orig_h))) =
                    (self.selected, self.resize_start, self.resize_orig)
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

                    match sel {
                        SelectedObject::Rect(i) => {
                            self.rects[i].resize(new_x, new_y, new_w, new_h);
                        }
                        SelectedObject::Polygon(i) => {
                            self.polygons[i].resize(new_x, new_y, new_w, new_h);
                        }
                    }
                    self.update_highlight();
                    self.update_inspector();
                }
            }
        }

        pub fn rotate_selected(&mut self, angle: f64) {
            if let Some(sel) = self.selected {
                match sel {
                    SelectedObject::Rect(i) => {
                        let new_rot = self.rects[i].rotation() + angle;
                        self.rects[i].set_rotation(new_rot);
                    }
                    SelectedObject::Polygon(i) => {
                        let new_rot = self.polygons[i].rotation() + angle;
                        self.polygons[i].set_rotation(new_rot);
                    }
                }
                self.update_highlight();
                self.update_inspector();
            }
        }

        pub fn handle_right_click(&mut self, x: f64, y: f64) {
            if let Some(ref mut pm) = self.polygon_menu {
                pm.close();
            }
            let mut menu = self.context_menu.take().unwrap_or_else(ContextMenu::new);
            menu.open(x, y);
            self.context_menu = Some(menu);
        }

        pub fn set_show_render_times(&mut self, show: bool) {
            self.show_render_times = show;
        }

        fn ensure_time_renderers(&mut self) {
            if self.show_render_times {
                if let Some(registry) = self.get_registry() {
                    ::hotline::set_library_registry(registry);
                }
                if self.rect_time_labels.len() != self.rects.len() {
                    self.rect_time_labels.resize_with(self.rects.len(), TextRenderer::new);
                }
                if self.polygon_time_labels.len() != self.polygons.len() {
                    self.polygon_time_labels.resize_with(self.polygons.len(), TextRenderer::new);
                }
            }
        }

        pub fn update_autonomy(&mut self, mouse_x: f64, mouse_y: f64) {
            for mover in &mut self.rect_movers {
                mover.update(mouse_x, mouse_y);
            }
            self.update_inspector();
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            self.ensure_time_renderers();

            // Render all rects
            for (i, rect_handle) in self.rects.iter_mut().enumerate() {
                let start = std::time::Instant::now();
                rect_handle.render(buffer, buffer_width, buffer_height, pitch);
                if self.show_render_times {
                    let us = start.elapsed().as_micros();
                    if let Some(label) = self.rect_time_labels.get_mut(i) {
                        label.set_text(format!("{}us", us));
                        let (x, y, _w, _h) = rect_handle.bounds();
                        let lh = label.line_height();
                        label.set_x(x);
                        label.set_y(y - lh);
                        label.render(buffer, buffer_width, buffer_height, pitch);
                    }
                }
            }

            // Render polygons
            for (i, poly) in self.polygons.iter_mut().enumerate() {
                let start = std::time::Instant::now();
                poly.render(buffer, buffer_width, buffer_height, pitch);
                if self.show_render_times {
                    let us = start.elapsed().as_micros();
                    if let Some(label) = self.polygon_time_labels.get_mut(i) {
                        label.set_text(format!("{}us", us));
                        let (x, y, _w, _h) = poly.bounds();
                        let lh = label.line_height();
                        label.set_x(x);
                        label.set_y(y - lh);
                        label.render(buffer, buffer_width, buffer_height, pitch);
                    }
                }
            }

            // Render images
            for image in &mut self.images {
                image.render(buffer, buffer_width, buffer_height, pitch);
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

            if let Some(ref mut pm) = self.polygon_menu {
                if pm.is_visible() {
                    pm.render(buffer, buffer_width, buffer_height, pitch);
                }
            }

            if let Some(ref mut inspector) = self.click_inspector {
                inspector.render(buffer, buffer_width, buffer_height, pitch);
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
