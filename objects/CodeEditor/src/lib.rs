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
        file_menu: Option<ContextMenu>,
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

        pub fn contains_point(&mut self, x: f64, y: f64) -> bool {
            if let Some(ref mut r) = self.rect { r.contains_point(x, y) } else { false }
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) {
            if let Some(ref mut menu) = self.file_menu {
                if menu.is_visible() {
                    if let Some(sel) = menu.handle_mouse_down(x, y) {
                        let path = format!("objects/{}/src/lib.rs", sel);
                        let _ = self.open(&path);
                    }
                    return;
                }
            }

            if let Some(ref mut r) = self.rect {
                self.focused = r.contains_point(x, y);
                if self.focused {
                    self.cursor = self.index_at_position(x, y);
                    self.selection = Some((self.cursor, self.cursor));
                    self.dragging = true;
                } else {
                    self.dragging = false;
                }
            }
        }

        pub fn handle_mouse_up(&mut self) {
            self.dragging = false;
        }

        pub fn handle_mouse_move(&mut self, x: f64, y: f64) {
            if self.dragging {
                let idx = self.index_at_position(x, y);
                self.cursor = idx;
                self.update_selection();
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

        pub fn open_file_menu(&mut self, x: f64, y: f64) -> Result<(), String> {
            let mut menu = self.file_menu.take().unwrap_or_else(ContextMenu::new);
            let mut items = Vec::new();
            for entry in std::fs::read_dir("objects").map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        items.push(name.to_string());
                    }
                }
            }
            items.sort();
            menu.set_items(items);
            menu.open(x, y);
            self.file_menu = Some(menu);
            Ok(())
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
            if let Some(ref mut tr) = self.text_renderer { tr.line_height() } else { 14.0 }
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

        fn index_at_position(&mut self, x: f64, y: f64) -> usize {
            if let Some(ref mut r) = self.rect {
                let (rx, ry, rw, rh) = r.bounds();
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
                let mut col = 0usize;
                let mut pos_x = 0.0;
                if let Some(ref mut tr) = self.text_renderer {
                    for ch in line_text.chars() {
                        let cw = tr.char_width(ch);
                        if local_x < pos_x + cw / 2.0 {
                            break;
                        }
                        pos_x += cw;
                        col += 1;
                    }
                    if local_x > pos_x {
                        col = line_text.chars().count();
                    }
                } else {
                    col = ((local_x / 8.0).round() as usize).min(line_text.chars().count());
                }
                self.line_start_index(line) + col
            } else {
                self.cursor
            }
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

        pub fn insert_newline(&mut self) {
            self.insert_char('\n');
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
                let line_height = 14.0;
                let total_height = self.text.lines().count() as f64 * line_height;
                let max_offset = (total_height - rect.bounds().3).max(0.0);
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
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            let line_height = self.line_height();
            let (x, y, w, h) = match self.rect.clone() {
                Some(mut r) => r.bounds(),
                None => return,
            };

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

            // Draw file name at top of the rect
            if let Some(ref mut tr) = self.text_renderer {
                if let Some(path) = &self.file_path {
                    let file_name = std::path::Path::new(path)
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| path.clone());
                    tr.set_text(file_name);
                    tr.set_x(x + 10.0);
                    tr.set_y(y + 2.0);
                    tr.render(buffer, buffer_width, buffer_height, pitch);
                }
            }

            if let Some((start, end)) = self.selection {
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
                            let x0 = x
                                + 10.0
                                + self
                                    .text_renderer
                                    .as_mut()
                                    .map(|tr| tr.measure_text(&line.chars().take(s_col).collect::<String>()))
                                    .unwrap_or(s_col as f64 * 8.0);
                            let x1 = x
                                + 10.0
                                + self
                                    .text_renderer
                                    .as_mut()
                                    .map(|tr| tr.measure_text(&line.chars().take(e_col).collect::<String>()))
                                    .unwrap_or(e_col as f64 * 8.0);
                            let px0 = x0.round() as i64;
                            let px1 = x1.round() as i64;
                            let py0 = line_y.round() as i64;
                            let py1 = (line_y + line_height).round() as i64;
                            for py in py0.max(0)..py1.min(buffer_height) {
                                for px in px0.max(0)..px1.min(buffer_width) {
                                    let off = (py * pitch + px * 4) as usize;
                                    if off + 3 < buffer.len() {
                                        buffer[off] = 60;
                                        buffer[off + 1] = 60;
                                        buffer[off + 2] = 120;
                                        buffer[off + 3] = 255;
                                    }
                                }
                            }
                        }
                    }
                    char_index += len + 1;
                    line_idx += 1;
                }
            }

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
            let line_text = self.text.split('\n').nth(line_idx).unwrap_or("");
            let prefix: String = line_text.chars().take(col_idx).collect();
            let col_px = self.text_renderer.as_mut().map(|tr| tr.measure_text(&prefix)).unwrap_or(col_idx as f64 * 8.0);
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
                    if let Some(rect) = self.rect.as_ref() {
                        *hl = hl.clone().with_target(rect);
                        hl.render(buffer, buffer_width, buffer_height, pitch);
                    }
                }
            }

            if let Some(ref mut menu) = self.file_menu {
                menu.render(buffer, buffer_width, buffer_height, pitch);
            }
        }
    }
});
