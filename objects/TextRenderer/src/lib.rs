hotline::object!({
    #[derive(Clone, Default)]
    pub struct TextRenderer {
        #[setter]
        text: String,
        #[setter]
        #[default(0.0)]
        x: f64,
        #[setter]
        #[default(0.0)]
        y: f64,
        #[setter]
        #[default((255, 255, 255, 255))]
        color: (u8, u8, u8, u8), // ABGR
        font: Option<Font>,
        initialized: bool,
    }

    impl TextRenderer {
        pub fn initialize(&mut self) {
            if self.initialized {
                return;
            }

            // Ensure registry is available for creating other objects
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            // Create loaders via the proxy system
            let mut json_loader = JSONLoader::new();

            // Load and parse font metadata
            if let Err(e) = json_loader.load_json("fonts/owlet/owlet.json") {
                panic!("Failed to load font JSON: {}", e);
            }

            // Create a Font object (ensure registry is still set)
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }
            let mut font = Font::new();

            // Parse JSON data into the Font object
            if let Err(e) = json_loader.parse_into(&mut font) {
                panic!("Failed to parse font data: {}", e);
            }

            self.font = Some(font);
            self.initialized = true;
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            // CPU rendering is not supported anymore - GPU only
            let _ = (buffer, buffer_width, buffer_height, pitch);
        }

        pub fn render_gpu(&mut self, gpu_renderer: &mut dyn ::hotline::GpuRenderingContext) {
            if !self.initialized {
                self.initialize();
            }

            let font = match &mut self.font {
                Some(f) => f,
                None => return,
            };

            // Use hardcoded font atlas ID 1
            let atlas_id = 1;

            let mut cursor_x = self.x;
            let cursor_y = self.y;
            let mut prev_char: Option<char> = None;

            // Convert color from ABGR u8 to RGBA f32
            let color = [
                self.color.2 as f32 / 255.0, // R
                self.color.1 as f32 / 255.0, // G
                self.color.0 as f32 / 255.0, // B
                self.color.3 as f32 / 255.0, // A
            ];

            // Hardcoded font atlas dimensions - these match owlet font atlas
            const ATLAS_WIDTH: f32 = 64.0;
            const ATLAS_HEIGHT: f32 = 128.0;

            for ch in self.text.chars() {
                // Apply kerning before rendering
                if let Some(prev) = prev_char {
                    cursor_x += font.kerning(prev, ch) as f64;
                }

                if ch == ' ' {
                    cursor_x += font.space_width() as f64;
                    prev_char = Some(ch);
                    continue;
                }

                if let Some((glyph_x, glyph_y, glyph_width, glyph_height, offset_x, offset_y, advance)) = font.glyph(ch)
                {
                    let dest_x = cursor_x + offset_x as f64;
                    let dest_y = cursor_y + offset_y as f64 + font.size() as f64;

                    // Calculate texture coordinates for the glyph
                    let u0 = glyph_x as f32 / ATLAS_WIDTH;
                    let v0 = glyph_y as f32 / ATLAS_HEIGHT;
                    let u1 = (glyph_x + glyph_width) as f32 / ATLAS_WIDTH;
                    let v1 = (glyph_y + glyph_height) as f32 / ATLAS_HEIGHT;

                    gpu_renderer.add_textured_rect_with_coords(
                        dest_x as f32,
                        dest_y as f32,
                        glyph_width as f32,
                        glyph_height as f32,
                        atlas_id,
                        u0,
                        v0,
                        u1,
                        v1,
                        color,
                    );

                    cursor_x += advance as f64;
                    prev_char = Some(ch);
                }
            }
        }

        pub fn char_width(&self, ch: char) -> f64 {
            let font = match &self.font {
                Some(f) => f,
                None => return 0.0,
            };

            if ch == ' ' {
                font.space_width() as f64
            } else if let Some((_x, _y, _w, _h, _off_x, _off_y, adv)) = font.glyph(ch) {
                adv as f64
            } else {
                font.space_width() as f64
            }
        }

        pub fn measure_text(&self, text: &str) -> f64 {
            let font = match &self.font {
                Some(f) => f,
                None => return 0.0,
            };

            let mut width = 0.0;
            let mut prev_char: Option<char> = None;

            for ch in text.chars() {
                if ch == ' ' {
                    width += font.space_width() as f64;
                } else if let Some((_x, _y, _w, _h, _off_x, _off_y, adv)) = font.glyph(ch) {
                    width += adv as f64;
                } else {
                    width += font.space_width() as f64;
                }

                // Apply kerning
                if let Some(prev) = prev_char {
                    width += font.kerning(prev, ch) as f64;
                }

                prev_char = Some(ch);
            }

            width
        }

        pub fn line_height(&self) -> f64 {
            if let Some(font) = self.font.as_ref() { (font.size() + font.line_gap()) as f64 } else { 14.0 }
        }
    }
});
