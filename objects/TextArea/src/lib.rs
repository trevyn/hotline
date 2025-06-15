hotline::object!({
    #[derive(Clone, Default)]
    pub struct TextArea {
        #[setter]
        text: String,
        rect: Option<Rect>,
        focused: bool,
        text_renderer: Option<TextRenderer>,
        cursor: usize,
        selection: Option<(usize, usize)>,
        #[default(false)]
        dragging: bool,
        #[setter]
        #[default((255, 255, 255, 255))]
        text_color: (u8, u8, u8, u8),
        #[setter]
        #[default(0.0)]
        scroll_offset: f64,
        #[default(0.0)]
        scroll_velocity: f64,
        #[setter]
        #[default(40)]
        background_color: u8,
        #[setter]
        #[default(true)]
        show_cursor: bool,
        #[setter]
        #[default(true)]
        editable: bool,
        background_atlas_id: Option<u32>,
        selection_atlas_id: Option<u32>,
        shared_white_atlas_id: Option<u32>,
    }

    impl TextArea {
        pub fn initialize(&mut self) {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            if self.text_renderer.is_none() {
                let mut tr = TextRenderer::new();
                tr.set_color(self.text_color);
                tr.initialize();
                self.text_renderer = Some(tr);
            }
            self.cursor = self.text.chars().count();
            self.selection = None;
        }

        pub fn set_rect(&mut self, rect: Rect) {
            self.rect = Some(rect);
            self.initialize();
        }

        pub fn update_text_color(&mut self, color: (u8, u8, u8, u8)) {
            self.text_color = color;
            if let Some(ref mut tr) = self.text_renderer {
                tr.set_color(color);
            }
        }

        pub fn is_focused(&self) -> bool {
            self.focused
        }

        pub fn set_focused(&mut self, focused: bool) {
            self.focused = focused;
        }

        pub fn contains_point(&mut self, x: f64, y: f64) -> bool {
            if let Some(ref r) = self.rect { r.clone().contains_point(x, y) } else { false }
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) -> bool {
            if let Some(ref r) = self.rect {
                self.focused = r.clone().contains_point(x, y);
                if self.focused && self.editable && self.is_near_text(x, y) {
                    self.cursor = self.index_at_position(x, y);
                    self.selection = Some((self.cursor, self.cursor));
                    self.dragging = true;
                    return true;
                } else {
                    self.dragging = false;
                }
            }
            false
        }

        pub fn handle_mouse_up(&mut self) {
            self.dragging = false;
        }

        pub fn handle_mouse_move(&mut self, x: f64, y: f64) {
            if self.dragging && self.editable {
                let idx = self.index_at_position(x, y);
                self.cursor = idx;
                self.update_selection();
            }
        }

        pub fn is_dragging(&self) -> bool {
            self.dragging
        }

        fn is_near_text(&self, x: f64, y: f64) -> bool {
            if let Some(ref r) = self.rect {
                let (rx, ry, _rw, _rh) = r.clone().bounds();
                let line_height = self.line_height();
                let local_y = y - (ry + 10.0) + self.scroll_offset;
                if local_y < 0.0 {
                    return false;
                }
                let line = (local_y / line_height).floor() as usize;
                let lines: Vec<&str> = self.text.split('\n').collect();
                if line >= lines.len() {
                    return false;
                }
                let line_y = ry + 10.0 + line as f64 * line_height - self.scroll_offset;
                if y < line_y - 2.0 || y > line_y + line_height + 2.0 {
                    return false;
                }
                let line_text = lines[line];
                let text_width = line_text.chars().count() as f64 * 8.0;
                let text_x0 = rx + 10.0;
                let text_x1 = text_x0 + text_width;
                x >= text_x0 - 5.0 && x <= text_x1 + 5.0
            } else {
                false
            }
        }

        pub fn get_text(&self) -> String {
            self.text.clone()
        }

        pub fn get_cursor(&self) -> usize {
            self.cursor
        }

        pub fn set_cursor(&mut self, cursor: usize) {
            self.cursor = cursor.min(self.text.chars().count());
        }

        pub fn get_selection(&self) -> Option<(usize, usize)> {
            self.selection
        }

        pub fn set_selection(&mut self, selection: Option<(usize, usize)>) {
            self.selection = selection;
        }

        pub fn clear_selection(&mut self) {
            self.selection = None;
        }

        fn char_to_byte(&self, idx: usize) -> usize {
            self.text.char_indices().nth(idx).map(|(b, _)| b).unwrap_or_else(|| self.text.len())
        }

        fn delete_range(&mut self, start: usize, end: usize) {
            let b_start = self.char_to_byte(start);
            let b_end = self.char_to_byte(end);
            self.text.replace_range(b_start..b_end, "");
        }

        fn line_height(&self) -> f64 {
            14.0
        }

        pub fn background_atlas_id(&self) -> Option<u32> {
            self.background_atlas_id
        }

        pub fn set_shared_white_atlas(&mut self, atlas_id: u32) {
            self.shared_white_atlas_id = Some(atlas_id);
        }

        fn index_to_line_col(&self, idx: usize) -> (usize, usize) {
            let mut line = 0usize;
            let mut col = 0usize;
            for ch in self.text.chars().take(idx) {
                if ch == '\n' {
                    line += 1;
                    col = 0;
                } else {
                    col += 1;
                }
            }
            (line, col)
        }

        fn cursor_line_col(&self) -> (usize, usize) {
            self.index_to_line_col(self.cursor)
        }

        fn line_start_index(&self, line: usize) -> usize {
            let mut idx = 0usize;
            for (i, l) in self.text.split('\n').enumerate() {
                if i == line {
                    break;
                }
                idx += l.chars().count() + 1;
            }
            idx
        }

        fn line_length(&self, line: usize) -> usize {
            self.text.split('\n').nth(line).map(|l| l.chars().count()).unwrap_or(0)
        }

        fn index_at_position(&self, x: f64, y: f64) -> usize {
            if let Some(ref r) = self.rect {
                let (rx, ry, rw, rh) = r.clone().bounds();
                let cx = x.clamp(rx, rx + rw);
                let cy = y.clamp(ry, ry + rh);
                let local_y = cy - (ry + 10.0) + self.scroll_offset;
                let line_height = self.line_height();
                let mut line = (local_y / line_height).floor() as usize;
                let lines: Vec<&str> = self.text.split('\n').collect();
                if line >= lines.len() {
                    line = lines.len().saturating_sub(1);
                }
                let local_x = cx - (rx + 10.0);
                let line_text = lines.get(line).copied().unwrap_or("");

                // Find character position by measuring text width
                let col = if let Some(ref tr) = self.text_renderer {
                    // Binary search approach to find the character position
                    let chars_vec: Vec<char> = line_text.chars().collect();
                    let mut left = 0;
                    let mut right = chars_vec.len();

                    while left < right {
                        let mid = (left + right + 1) / 2;
                        let text_to_mid: String = chars_vec[..mid].iter().collect();
                        let width = tr.measure_text(&text_to_mid);

                        if width <= local_x {
                            left = mid;
                        } else {
                            right = mid - 1;
                        }
                    }

                    // Check if we're closer to the next character
                    if left < chars_vec.len() {
                        let text_to_left: String = chars_vec[..left].iter().collect();
                        let text_to_next: String = chars_vec[..=left].iter().collect();
                        let width_left = if left == 0 { 0.0 } else { tr.measure_text(&text_to_left) };
                        let width_next = tr.measure_text(&text_to_next);

                        if local_x - width_left > (width_next - width_left) / 2.0 { left + 1 } else { left }
                    } else {
                        left
                    }
                } else {
                    // Fallback to fixed width if text renderer not available
                    ((local_x / 8.0).round() as usize).min(line_text.chars().count())
                };

                self.line_start_index(line) + col
            } else {
                self.cursor
            }
        }

        pub fn insert_char(&mut self, ch: char) {
            if self.focused && self.editable {
                if let Some((s, e)) = self.selection.take() {
                    self.delete_range(s.min(e), s.max(e));
                    self.cursor = s.min(e);
                }
                let b = self.char_to_byte(self.cursor);
                self.text.insert(b, ch);
                self.cursor += 1;
            }
        }

        pub fn insert_text(&mut self, text: &str) {
            if self.focused && self.editable {
                if let Some((s, e)) = self.selection.take() {
                    self.delete_range(s.min(e), s.max(e));
                    self.cursor = s.min(e);
                }
                let b = self.char_to_byte(self.cursor);
                self.text.insert_str(b, text);
                self.cursor += text.chars().count();
            }
        }

        pub fn insert_newline(&mut self) {
            self.insert_char('\n');
        }

        pub fn backspace(&mut self) {
            if self.focused && self.editable {
                if let Some((s, e)) = self.selection.take() {
                    self.delete_range(s.min(e), s.max(e));
                    self.cursor = s.min(e);
                } else if self.cursor > 0 {
                    let b_start = self.char_to_byte(self.cursor - 1);
                    let b_end = self.char_to_byte(self.cursor);
                    self.text.replace_range(b_start..b_end, "");
                    self.cursor -= 1;
                }
            }
        }

        pub fn move_cursor_left(&mut self, shift: bool) {
            if self.cursor > 0 {
                self.cursor -= 1;
            }
            if shift {
                self.update_selection();
            } else {
                self.selection = None;
            }
        }

        pub fn move_cursor_right(&mut self, shift: bool) {
            if self.cursor < self.text.chars().count() {
                self.cursor += 1;
            }
            if shift {
                self.update_selection();
            } else {
                self.selection = None;
            }
        }

        pub fn move_cursor_up(&mut self, shift: bool) {
            let (line, col) = self.cursor_line_col();
            if line == 0 {
                if shift {
                    self.update_selection();
                } else {
                    self.selection = None;
                }
                return;
            }
            let new_line = line - 1;
            let new_col = col.min(self.line_length(new_line));
            self.cursor = self.line_start_index(new_line) + new_col;
            if shift {
                self.update_selection();
            } else {
                self.selection = None;
            }
        }

        pub fn move_cursor_down(&mut self, shift: bool) {
            let (line, col) = self.cursor_line_col();
            let total_lines = self.text.lines().count();
            if line + 1 >= total_lines {
                if shift {
                    self.update_selection();
                } else {
                    self.selection = None;
                }
                return;
            }
            let new_line = line + 1;
            let new_col = col.min(self.line_length(new_line));
            self.cursor = self.line_start_index(new_line) + new_col;
            if shift {
                self.update_selection();
            } else {
                self.selection = None;
            }
        }

        fn update_selection(&mut self) {
            match self.selection {
                Some((start, _)) => self.selection = Some((start, self.cursor)),
                None => self.selection = Some((self.cursor, self.cursor)),
            }
        }

        pub fn scroll_by(&mut self, delta: f64) {
            if let Some(ref rect) = self.rect {
                let line_height = 14.0;
                let total_height = self.text.lines().count() as f64 * line_height;
                let max_offset = (total_height - rect.clone().bounds().3).max(0.0);
                self.scroll_offset = (self.scroll_offset + delta).max(0.0).min(max_offset);
            }
        }

        pub fn add_scroll_velocity(&mut self, delta: f64) {
            self.scroll_velocity += delta;
        }

        pub fn update_scroll(&mut self) {
            if self.scroll_velocity.abs() > 0.1 {
                self.scroll_by(self.scroll_velocity);
                self.scroll_velocity *= 0.85;
            } else {
                self.scroll_velocity = 0.0;
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            // GPU only - no CPU rendering
            let _ = (buffer, buffer_width, buffer_height, pitch);
        }

        pub fn register_atlases(&mut self, gpu_renderer: &mut GPURenderer) {
            // Register background atlas
            if self.background_atlas_id.is_none() {
                let bg_pixel = vec![self.background_color, self.background_color, self.background_color, 255];
                let id = gpu_renderer.register_atlas(bg_pixel, 1, 1, AtlasFormat::RGBA);
                self.background_atlas_id = Some(id);
            }

            // Register selection atlas
            if self.selection_atlas_id.is_none() {
                let sel_pixel = vec![60, 60, 120, 255];
                let id = gpu_renderer.register_atlas(sel_pixel, 1, 1, AtlasFormat::RGBA);
                self.selection_atlas_id = Some(id);
            }

            // Register text renderer atlas
            if let Some(ref mut tr) = self.text_renderer {
                tr.register_atlas(gpu_renderer);
            }
        }

        pub fn generate_commands(&mut self, gpu_renderer: &mut GPURenderer) {
            let (x, y, w, h) = match self.rect.as_ref() {
                Some(r) => r.clone().bounds(),
                None => return,
            };

            let scroll_bar_width = 8.0;

            // Render background using shared white atlas if available
            let bg_atlas = self.shared_white_atlas_id.or(self.background_atlas_id);
            if let Some(bg_id) = bg_atlas {
                let bg_color = if self.shared_white_atlas_id.is_some() {
                    // Use color modulation with white atlas
                    (self.background_color, self.background_color, self.background_color, 255)
                } else {
                    // Use pre-colored atlas
                    (255, 255, 255, 255)
                };

                gpu_renderer.add_command(RenderCommand::Rect {
                    texture_id: bg_id,
                    dest_x: x,
                    dest_y: y,
                    dest_width: w - scroll_bar_width,
                    dest_height: h,
                    rotation: 0.0,
                    color: bg_color,
                });
            }

            // Render selection boxes
            if let Some((start, end)) = self.selection {
                if let Some(sel_id) = self.selection_atlas_id {
                    let line_height = self.line_height();
                    let text = self.text.clone();
                    let (start, end) = if start <= end { (start, end) } else { (end, start) };
                    let mut char_index = 0usize;
                    let mut line_idx = 0usize;

                    for line in text.split('\n') {
                        let len = line.chars().count();
                        let line_start = char_index;
                        let line_end = char_index + len;

                        if line_end >= start && line_start <= end {
                            let s_col = if start > line_start { start - line_start } else { 0 };
                            let e_col = if end < line_end { end - line_start } else { len };
                            let line_y = y + 10.0 + line_idx as f64 * line_height - self.scroll_offset;

                            if line_y + line_height >= y && line_y <= y + h {
                                let x0 = if let Some(ref tr) = self.text_renderer {
                                    let chars: Vec<char> = line.chars().collect();
                                    let text_to_start: String = chars[..s_col].iter().collect();
                                    x + 10.0 + if s_col == 0 { 0.0 } else { tr.measure_text(&text_to_start) }
                                } else {
                                    x + 10.0 + s_col as f64 * 8.0
                                };

                                let x1 = if let Some(ref tr) = self.text_renderer {
                                    let chars: Vec<char> = line.chars().collect();
                                    let text_to_end: String = chars[..e_col].iter().collect();
                                    x + 10.0 + if e_col == 0 { 0.0 } else { tr.measure_text(&text_to_end) }
                                } else {
                                    x + 10.0 + e_col as f64 * 8.0
                                };

                                gpu_renderer.add_command(RenderCommand::Rect {
                                    texture_id: sel_id,
                                    dest_x: x0,
                                    dest_y: line_y,
                                    dest_width: x1 - x0,
                                    dest_height: line_height,
                                    rotation: 0.0,
                                    color: (255, 255, 255, 255),
                                });
                            }
                        }
                        char_index += len + 1;
                        line_idx += 1;
                    }
                }
            }

            // Generate text commands
            let mut cursor_y = y + 10.0 - self.scroll_offset;
            let line_height = self.line_height();
            if let Some(ref mut tr) = self.text_renderer {
                for line in self.text.split('\n') {
                    if cursor_y + line_height >= y && cursor_y <= y + h {
                        tr.set_text(line.to_string());
                        tr.set_x(x + 10.0);
                        tr.set_y(cursor_y);
                        tr.generate_commands(gpu_renderer);
                    }
                    cursor_y += line_height;
                }
            }
        }
    }
});
