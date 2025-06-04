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
        color: (u8, u8, u8, u8), // BGRA
        font: Option<Font>,
        atlas: Vec<u8>,
        atlas_width: u32,
        atlas_height: u32,
        initialized: bool,
    }

    impl TextRenderer {
        pub fn initialize(&mut self) {
            if self.initialized {
                return;
            }

            // Create loaders via the proxy system
            let mut json_loader = JSONLoader::new();
            let mut png_loader = PNGLoader::new();

            // Load and parse font metadata
            if let Err(e) = json_loader.load_json("fonts/owlet/owlet.json") {
                panic!("Failed to load font JSON: {}", e);
            }

            // Create a Font object
            let mut font = Font::new();

            // Parse JSON data into the Font object
            if let Err(e) = json_loader.parse_into(&mut font) {
                panic!("Failed to parse font data: {}", e);
            }

            // Font loaded successfully

            // Load atlas
            if let Err(e) = png_loader.load_png("fonts/owlet/owlet.png") {
                panic!("Failed to load font PNG: {}", e);
            }

            if let Some((atlas_data, width, height)) = png_loader.data() {
                let _data_len = atlas_data.len();
                self.atlas = atlas_data;
                self.atlas_width = width;
                self.atlas_height = height;
                // Atlas loaded successfully
            } else {
                panic!("Failed to get atlas data from PNG loader");
            }

            self.font = Some(font);
            self.initialized = true;
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if !self.initialized {
                self.initialize();
            }

            let font = match &mut self.font {
                Some(f) => f,
                None => return,
            };

            let mut cursor_x = self.x;
            let cursor_y = self.y;

            let (b, g, r, a) = self.color;

            for ch in self.text.chars() {
                if ch == ' ' {
                    cursor_x += font.space_width() as f64;
                    continue;
                }

                if let Some((glyph_x, glyph_y, glyph_width, glyph_height, offset_x, offset_y, advance)) = font.glyph(ch)
                {
                    // Render glyph
                    // Calculate destination position
                    let dest_x = cursor_x + offset_x as f64;
                    let dest_y = cursor_y + offset_y as f64 + font.size() as f64;

                    // Render glyph
                    for py in 0..glyph_height {
                        for px in 0..glyph_width {
                            let src_x = glyph_x + px;
                            let src_y = glyph_y + py;

                            // The PNG is GrayscaleAlpha format (2 bytes per pixel)
                            let src_offset = ((src_y * self.atlas_width + src_x) * 2) as usize;
                            if src_offset + 1 < self.atlas.len() {
                                // Get gray value and alpha
                                let gray = self.atlas[src_offset];
                                let alpha = self.atlas[src_offset + 1];

                                // For grayscale fonts, use the gray value as the opacity
                                // (some fonts use gray for shape, others use alpha)
                                let opacity = (gray as u32 * alpha as u32) / 255;

                                if opacity > 0 {
                                    let screen_x = (dest_x + px as f64) as i32;
                                    let screen_y = (dest_y + py as f64) as i32;

                                    if screen_x >= 0
                                        && screen_x < buffer_width as i32
                                        && screen_y >= 0
                                        && screen_y < buffer_height as i32
                                    {
                                        let dest_offset =
                                            (screen_y as u32 * pitch as u32 + screen_x as u32 * 4) as usize;
                                        if dest_offset + 3 < buffer.len() {
                                            // Apply text color with alpha blending
                                            let src_alpha = (opacity * a as u32) / 255;
                                            let inv_alpha = 255 - src_alpha;

                                            buffer[dest_offset] =
                                                ((b as u32 * src_alpha + buffer[dest_offset] as u32 * inv_alpha) / 255)
                                                    as u8;
                                            buffer[dest_offset + 1] =
                                                ((g as u32 * src_alpha + buffer[dest_offset + 1] as u32 * inv_alpha)
                                                    / 255) as u8;
                                            buffer[dest_offset + 2] =
                                                ((r as u32 * src_alpha + buffer[dest_offset + 2] as u32 * inv_alpha)
                                                    / 255) as u8;
                                            buffer[dest_offset + 3] =
                                                255.min(buffer[dest_offset + 3] + src_alpha as u8);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    cursor_x += advance as f64;
                }
            }
        }
    }
});
