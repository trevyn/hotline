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
                self.text_renderer = Some(TextRenderer::new());
            }

            self.cursor = self.text.chars().count();
            self.selection = None;
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

        fn update_selection(&mut self) {
            match self.selection {
                Some((start, _)) => self.selection = Some((start, self.cursor)),
                None => self.selection = Some((self.cursor, self.cursor)),
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            if let Some(ref mut rect) = self.rect {
                let (x, y, w, h) = rect.bounds();
                let x_start = x.max(0.0) as u32;
                let y_start = y.max(0.0) as u32;
                let x_end = (x + w).min(buffer_width as f64) as u32;
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

                let mut cursor_y = y + 10.0;
                let line_height = 14.0;

                if let Some(ref mut tr) = self.text_renderer {
                    tr.set_color((255, 255, 255, 255));
                    for line in self.text.split('\n') {
                        tr.set_text(line.to_string());
                        tr.set_x(x + 10.0);
                        tr.set_y(cursor_y);
                        tr.render(buffer, buffer_width, buffer_height, pitch);
                        cursor_y += line_height;
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
