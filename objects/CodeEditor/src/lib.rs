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
    }

    impl CodeEditor {
        pub fn initialize(&mut self) {
            if self.highlight.is_none() {
                self.highlight = Some(HighlightLens::new());
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
            self.text = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read {}: {}", path, e))?;
            self.file_path = Some(path.to_string());
            self.initialize();
            Ok(())
        }

        pub fn save(&mut self) -> Result<(), String> {
            if let Some(path) = &self.file_path {
                std::fs::write(path, &self.text)
                    .map_err(|e| format!("Failed to write {}: {}", path, e))?;
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

        pub fn compile_and_reload(&mut self, lib_name: &str) -> Result<(), String> {
            let status = std::process::Command::new("cargo")
                .args(["build", "--release", "-p", lib_name])
                .status()
                .map_err(|e| format!("Failed to run cargo: {}", e))?;
            if !status.success() {
                return Err("cargo build failed".into());
            }

            if let Some(registry) = self.get_registry() {
                #[cfg(target_os = "macos")]
                let lib_path = format!("target/release/lib{}.dylib", lib_name);
                #[cfg(target_os = "linux")]
                let lib_path = format!("target/release/lib{}.so", lib_name);
                #[cfg(target_os = "windows")]
                let lib_path = format!("target/release/{}.dll", lib_name);

                registry.load(&lib_path).map_err(|e| format!("Failed to reload {}: {}", lib_name, e))?;
                Ok(())
            } else {
                Err("registry not set".into())
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
                for line in self.text.split('\n') {
                    let mut tr = TextRenderer::new()
                        .with_text(line.to_string())
                        .with_x(x + 10.0)
                        .with_y(cursor_y)
                        .with_color((255, 255, 255, 255));
                    tr.render(buffer, buffer_width, buffer_height, pitch);
                    cursor_y += line_height;
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

