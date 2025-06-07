use hotline::HotlineObject;

hotline::object!({
    #[derive(Clone, Default)]
    pub struct CodeEditor {
        #[setter]
        text: String,
        file_path: Option<String>,
        rect: Option<Rect>,
        focused: bool,
        highlight: Option<HighlightLens>,
        text_renderer: Option<TextRenderer>,
        cursor: usize,
        selection: Option<(usize, usize)>,
        #[setter]
        #[default((255, 255, 255, 255))]
        text_color: (u8, u8, u8, u8),
        #[setter]
        #[default(0.0)]
        scroll_offset: f64,
    }

    impl CodeEditor {
        pub fn initialize(&mut self) {
            // Ensure the thread-local registry is set before creating other objects
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            if self.highlight.is_none() {
                self.highlight = Some(HighlightLens::new());
            }

            if self.text_renderer.is_none() {
                let mut tr = TextRenderer::new();
                tr.set_color(self.text_color);
                self.text_renderer = Some(tr);
            }
            self.cursor = self.text.chars().count();
            self.selection = None;
        }

        pub fn update_text_color(&mut self, color: (u8, u8, u8, u8)) {
            self.text_color = color;
            if let Some(ref mut tr) = self.text_renderer {
                tr.set_color(color);
            }
        }

        pub fn set_rect(&mut self, rect: Rect) {
            self.rect = Some(rect);
            self.initialize();
        }

        pub fn is_focused(&mut self) -> bool {
            self.focused
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) {
            if let Some(ref mut r) = self.rect {
                self.focused = r.contains_point(x, y);
            }
        }

        pub fn open(&mut self, path: &str) -> Result<(), String> {
            self.text = std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))?;
            self.file_path = Some(path.to_string());
            self.initialize();
            Ok(())
        }

        pub fn save(&mut self) -> Result<(), String> {
            if let Some(path) = &self.file_path {
                std::fs::write(path, &self.text).map_err(|e| format!("Failed to write {}: {}", path, e))?;
                Ok(())
            } else {
                Err("no file loaded".into())
            }
        }

        fn char_to_byte(&self, idx: usize) -> usize {
            self.text.char_indices().nth(idx).map(|(b, _)| b).unwrap_or_else(|| self.text.len())
        }

        fn delete_range(&mut self, start: usize, end: usize) {
            let b_start = self.char_to_byte(start);
            let b_end = self.char_to_byte(end);
            self.text.replace_range(b_start..b_end, "");
        }

        fn line_height(&mut self) -> f64 {
            if let Some(ref mut tr) = self.text_renderer {
                if let Some(font) = &mut tr.font {
                    return (font.size + font.line_gap) as f64;
                }
            }
            14.0
        }

        fn cursor_line_col(&self) -> (usize, usize) {
            let mut line = 0usize;
            let mut col = 0usize;
            for ch in self.text.chars().take(self.cursor) {
                if ch == '\n' {
                    line += 1;
                    col = 0;
                } else {
                    col += 1;
                }
            }
            (line, col)
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

        fn column_to_pixel(&mut self, line: usize, col: usize) -> f64 {
            if let Some(ref mut tr) = self.text_renderer {
                if let Some(font) = &mut tr.font {
                    let mut current_line = 0usize;
                    let mut current_col = 0usize;
                    let mut px = 0.0;
                    for ch in self.text.chars() {
                        if current_line == line {
                            if current_col == col {
                                break;
                            }
                            if ch == '\n' {
                                break;
                            }
                            if ch == ' ' {
                                px += font.space_width as f64;
                            } else if let Some((_, _, _, _, _, _, adv)) = font.glyph(ch) {
                                px += adv as f64;
                            } else {
                                px += font.space_width as f64;
                            }
                            current_col += 1;
                        }

                        if ch == '\n' {
                            if current_line == line {
                                break;
                            }
                            current_line += 1;
                        }
                    }
                    return px;
                }
            }
            col as f64 * 8.0
        }

        pub fn insert_char(&mut self, ch: char) {
            if self.focused {
                if let Some((s, e)) = self.selection.take() {
                    self.delete_range(s.min(e), s.max(e));
                    self.cursor = s.min(e);
                }
                let b = self.char_to_byte(self.cursor);
                self.text.insert(b, ch);
                self.cursor += 1;
            }
        }

        pub fn backspace(&mut self) {
            if self.focused {
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
            if let Some(ref mut rect) = self.rect {
                let line_height = self.line_height();
                let total_height = self.text.lines().count() as f64 * line_height;
                let max_offset = (total_height - rect.bounds().3).max(0.0);
                self.scroll_offset = (self.scroll_offset + delta).max(0.0).min(max_offset);
            }
        }

        pub fn scroll_by(&mut self, delta: f64) {
            if let Some(ref mut rect) = self.rect {
                let line_height = 14.0;
                let total_height = self.text.lines().count() as f64 * line_height;
                let max_offset = (total_height - rect.bounds().3).max(0.0);
                self.scroll_offset = (self.scroll_offset + delta).max(0.0).min(max_offset);
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            if let Some(ref mut rect) = self.rect {
                let (x, y, w, h) = rect.bounds();
                let scroll_bar_width = 8.0;
                let x_start = x.max(0.0) as u32;
                let y_start = y.max(0.0) as u32;
                let x_end = (x + w - scroll_bar_width).min(buffer_width as f64) as u32;
                let y_end = (y + h).min(buffer_height as f64) as u32;

                // Draw background
                for py in y_start..y_end {
                    for px in x_start..x_end {
                        let offset = (py * (pitch as u32) + px * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = 40;
                            buffer[offset + 1] = 40;
                            buffer[offset + 2] = 40;
                            buffer[offset + 3] = 255;
                        }
                    }
                }

                let line_height = self.line_height();

                let mut cursor_y = y + 10.0 - self.scroll_offset;

                if let Some(ref mut tr) = self.text_renderer {
                    tr.set_color(self.text_color);
                    for line in self.text.split('\n') {
                        if cursor_y + line_height >= y && cursor_y <= y + h {
                            tr.set_text(line.to_string());
                            tr.set_x(x + 10.0);
                            tr.set_y(cursor_y);
                            tr.render(buffer, buffer_width, buffer_height, pitch);
                        }
                        cursor_y += line_height;
                    }
                }

                // Draw text cursor
                let (line_idx, col_idx) = self.cursor_line_col();
                let col_px = self.column_to_pixel(line_idx, col_idx);
                let cursor_x = x + 10.0 + col_px;
                let cursor_y_pos = y + 10.0 + line_idx as f64 * line_height - self.scroll_offset;
                if cursor_y_pos >= y && cursor_y_pos <= y + h {
                    let px = cursor_x.round() as i64;
                    let py_start = cursor_y_pos.round() as i64;
                    let py_end = (cursor_y_pos + line_height).round() as i64;
                    for py in py_start.max(0)..py_end.min(buffer_height) {
                        if px >= 0 && px < buffer_width {
                            let offset = (py * pitch + px * 4) as usize;
                            if offset + 3 < buffer.len() {
                                buffer[offset] = 200;
                                buffer[offset + 1] = 200;
                                buffer[offset + 2] = 200;
                                buffer[offset + 3] = 255;
                            }
                        }
                    }
                }
                let total_height = self.text.lines().count() as f64 * line_height;
                if total_height > h {
                    let bar_height = (h / total_height) * h;
                    let bar_y = y + (self.scroll_offset / total_height) * h;
                    let bar_x_start = (x + w - scroll_bar_width).max(0.0) as u32;
                    let bar_x_end = (x + w).min(buffer_width as f64) as u32;
                    let bar_y_start = bar_y.max(0.0) as u32;
                    let bar_y_end = (bar_y + bar_height).min(buffer_height as f64).min(y + h) as u32;

                    for py in bar_y_start..bar_y_end {
                        for px in bar_x_start..bar_x_end {
                            let offset = (py * (pitch as u32) + px * 4) as usize;
                            if offset + 3 < buffer.len() {
                                buffer[offset] = 80;
                                buffer[offset + 1] = 80;
                                buffer[offset + 2] = 80;
                                buffer[offset + 3] = 255;
                            }
                        }
                    }
                }

                if self.focused {
                    if let Some(ref mut hl) = self.highlight {
                        *hl = hl.clone().with_target(&*rect);
                        hl.render(buffer, buffer_width, buffer_height, pitch);
                    }
                }
            }
        }
    }
});
