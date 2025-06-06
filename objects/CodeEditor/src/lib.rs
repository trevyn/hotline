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

        pub fn insert_char(&mut self, ch: char) {
            if self.focused {
                self.text.push(ch);
            }
        }

        pub fn backspace(&mut self) {
            if self.focused {
                self.text.pop();
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

                let line_height = 14.0;

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
