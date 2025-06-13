hotline::object!({
    #[derive(Clone, Default)]
    pub struct CodeEditor {
        file_path: Option<String>,
        file_name: Option<String>,
        rect: Option<Rect>,
        highlight: Option<HighlightLens>,
        text_area: Option<TextArea>,
        file_menu: Option<ContextMenu>,
    }

    impl CodeEditor {
        pub fn initialize(&mut self) {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            if self.highlight.is_none() {
                self.highlight = Some(HighlightLens::new());
            }

            if self.text_area.is_none() {
                let mut ta = TextArea::new();
                ta.set_editable(true);
                self.text_area = Some(ta);
            }
        }

        pub fn set_rect(&mut self, rect: Rect) {
            self.rect = Some(rect.clone());
            self.initialize();
            if let Some(ref mut ta) = self.text_area {
                ta.set_rect(rect);
            }
        }

        pub fn is_focused(&mut self) -> bool {
            if let Some(ref mut ta) = self.text_area { ta.is_focused() } else { false }
        }

        pub fn contains_point(&mut self, x: f64, y: f64) -> bool {
            if let Some(ref mut ta) = self.text_area { ta.contains_point(x, y) } else { false }
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) -> bool {
            if let Some(ref mut menu) = self.file_menu {
                if menu.is_visible() {
                    if let Some(sel) = menu.handle_mouse_down(x, y) {
                        let path = format!("objects/{}/src/lib.rs", sel);
                        let _ = self.open(&path);
                    }
                    return true;
                }
            }

            if let Some(ref mut ta) = self.text_area { ta.handle_mouse_down(x, y) } else { false }
        }

        pub fn handle_mouse_up(&mut self) {
            if let Some(ref mut ta) = self.text_area {
                ta.handle_mouse_up();
            }
        }

        pub fn handle_mouse_move(&mut self, x: f64, y: f64) {
            if let Some(ref mut ta) = self.text_area {
                ta.handle_mouse_move(x, y);
            }
        }

        pub fn is_dragging(&mut self) -> bool {
            if let Some(ref mut ta) = self.text_area { ta.is_dragging() } else { false }
        }

        pub fn open(&mut self, path: &str) -> Result<(), String> {
            let text = std::fs::read_to_string(path).map_err(|e| format!("Failed to read {}: {}", path, e))?;
            self.file_path = Some(path.to_string());

            let p = std::path::Path::new(path);
            self.file_name = p
                .strip_prefix("objects")
                .ok()
                .map(|p| p.to_string_lossy().into_owned())
                .or_else(|| p.file_name().map(|s| s.to_string_lossy().into_owned()));

            self.initialize();
            if let Some(ref mut ta) = self.text_area {
                ta.set_text(text);
            }
            Ok(())
        }

        pub fn save(&mut self) -> Result<(), String> {
            if let Some(path) = &self.file_path {
                if let Some(ref mut ta) = self.text_area {
                    let text = ta.get_text();
                    std::fs::write(path, &text).map_err(|e| format!("Failed to write {}: {}", path, e))?;
                }
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

        pub fn insert_char(&mut self, ch: char) {
            if let Some(ref mut ta) = self.text_area {
                ta.insert_char(ch);
            }
        }

        pub fn insert_newline(&mut self) {
            if let Some(ref mut ta) = self.text_area {
                ta.insert_newline();
            }
        }

        pub fn backspace(&mut self) {
            if let Some(ref mut ta) = self.text_area {
                ta.backspace();
            }
        }

        pub fn move_cursor_left(&mut self, shift: bool) {
            if let Some(ref mut ta) = self.text_area {
                ta.move_cursor_left(shift);
            }
        }

        pub fn move_cursor_right(&mut self, shift: bool) {
            if let Some(ref mut ta) = self.text_area {
                ta.move_cursor_right(shift);
            }
        }

        pub fn move_cursor_up(&mut self, shift: bool) {
            if let Some(ref mut ta) = self.text_area {
                ta.move_cursor_up(shift);
            }
        }

        pub fn move_cursor_down(&mut self, shift: bool) {
            if let Some(ref mut ta) = self.text_area {
                ta.move_cursor_down(shift);
            }
        }

        pub fn scroll_by(&mut self, delta: f64) {
            if let Some(ref mut ta) = self.text_area {
                ta.scroll_by(delta);
            }
        }

        pub fn add_scroll_velocity(&mut self, delta: f64) {
            if let Some(ref mut ta) = self.text_area {
                ta.add_scroll_velocity(delta);
            }
        }

        pub fn update_scroll(&mut self) {
            if let Some(ref mut ta) = self.text_area {
                ta.update_scroll();
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            if let Some(ref mut ta) = self.text_area {
                ta.render(buffer, buffer_width, buffer_height, pitch);
            }

            // Draw file name at top of the rect
            if let Some(ref rect) = self.rect {
                let (x, y, _w, _h) = rect.clone().bounds();
                if let Some(name) = self.file_name.clone().or_else(|| {
                    self.file_path.as_ref().map(|path| {
                        std::path::Path::new(path)
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.clone())
                    })
                }) {
                    if let Some(registry) = self.get_registry() {
                        ::hotline::set_library_registry(registry);
                    }
                    let mut tr = TextRenderer::new();
                    tr.set_text(name);
                    tr.set_x(x + 10.0);
                    tr.set_y(y + 2.0);
                    tr.render(buffer, buffer_width, buffer_height, pitch);
                }
            }

            // Draw highlight
            if self.is_focused() {
                if let Some(ref mut hl) = self.highlight {
                    if let Some(rect) = self.rect.as_ref() {
                        *hl = hl.clone().with_target(rect);
                        hl.render(buffer, buffer_width, buffer_height, pitch);
                    }
                }
            }

            // Draw file menu
            if let Some(ref mut menu) = self.file_menu {
                menu.render(buffer, buffer_width, buffer_height, pitch);
            }
        }

        // Delegation methods for convenience
        pub fn set_text(&mut self, text: String) {
            if let Some(ref mut ta) = self.text_area {
                ta.set_text(text);
            }
        }

        pub fn update_text_color(&mut self, color: (u8, u8, u8, u8)) {
            if let Some(ref mut ta) = self.text_area {
                ta.update_text_color(color);
            }
        }
    }
});
