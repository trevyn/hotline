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
        atlas: Vec<u8>,
        atlas_width: u32,
        atlas_height: u32,
        initialized: bool,
        atlas_id: Option<u32>,
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
            let mut png_loader = PNGLoader::new();

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

            let (a, b, g, r) = self.color;
            let mut prev_char: Option<char> = None;

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
                    prev_char = Some(ch);
                }
            }
        }

        pub fn register_atlas(&mut self, gpu_renderer: &mut GPURenderer) {
            if !self.initialized {
                self.initialize();
            }

            if self.atlas_id.is_none() && !self.atlas.is_empty() {
                let id = gpu_renderer.register_atlas(
                    self.atlas.clone(),
                    self.atlas_width,
                    self.atlas_height,
                    AtlasFormat::GrayscaleAlpha,
                );
                self.atlas_id = Some(id);
            }
        }

        pub fn generate_commands(&mut self, gpu_renderer: &mut GPURenderer) {
            if !self.initialized {
                self.initialize();
            }

            let font = match &mut self.font {
                Some(f) => f,
                None => return,
            };

            let atlas_id = match self.atlas_id {
                Some(id) => id,
                None => {
                    return;
                }
            };

            let mut cursor_x = self.x;
            let cursor_y = self.y;
            let mut prev_char: Option<char> = None;

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

                    gpu_renderer.add_command(RenderCommand::Atlas {
                        texture_id: atlas_id,
                        src_x: glyph_x,
                        src_y: glyph_y,
                        src_width: glyph_width,
                        src_height: glyph_height,
                        dest_x,
                        dest_y,
                        color: self.color, // Already in ABGR order
                    });

                    cursor_x += advance as f64;
                    prev_char = Some(ch);
                }
            }
        }

        pub fn render_gpu(&mut self, gpu_renderer: &mut dyn ::hotline::GpuRenderingContext) {
            if !self.initialized {
                self.initialize();
            }

            let font = match &mut self.font {
                Some(f) => f,
                None => return,
            };

            // Create texture atlas if needed
            if self.atlas_id.is_none() && !self.atlas.is_empty() {
                // Convert grayscale alpha to RGBA
                // Input is 2 bytes per pixel (gray, alpha), output is 4 bytes per pixel (RGBA)
                let pixel_count = self.atlas.len() / 2;
                let mut rgba_atlas = Vec::with_capacity(pixel_count * 4);

                // Debug logging
                static mut LOGGED: bool = false;
                unsafe {
                    if !LOGGED {
                        eprintln!("TextRenderer: Creating font atlas texture");
                        eprintln!("  Atlas size: {}x{}", self.atlas_width, self.atlas_height);
                        eprintln!("  Input atlas len: {}, pixel count: {}", self.atlas.len(), pixel_count);
                        LOGGED = true;
                    }
                }

                for i in (0..self.atlas.len()).step_by(2) {
                    let gray = self.atlas[i];
                    let alpha = self.atlas[i + 1];
                    // Use the gray channel as the texture color and alpha as transparency
                    // This way the shader can tint it with the vertex color
                    rgba_atlas.push(gray); // R
                    rgba_atlas.push(gray); // G
                    rgba_atlas.push(gray); // B
                    rgba_atlas.push(alpha); // A
                }

                match gpu_renderer.create_rgba_texture(&rgba_atlas, self.atlas_width, self.atlas_height) {
                    Ok(id) => {
                        self.atlas_id = Some(id);
                        eprintln!("TextRenderer: Created texture atlas with ID {}", id);
                    }
                    Err(e) => {
                        eprintln!("Failed to create text atlas: {}", e);
                        return;
                    }
                }
            }

            let atlas_id = match self.atlas_id {
                Some(id) => id,
                None => return,
            };

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
                    let u0 = glyph_x as f32 / self.atlas_width as f32;
                    let v0 = glyph_y as f32 / self.atlas_height as f32;
                    let u1 = (glyph_x + glyph_width) as f32 / self.atlas_width as f32;
                    let v1 = (glyph_y + glyph_height) as f32 / self.atlas_height as f32;

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

        pub fn atlas_data(&self) -> Vec<u8> {
            self.atlas.clone()
        }

        pub fn atlas_dimensions(&self) -> (u32, u32) {
            (self.atlas_width, self.atlas_height)
        }

        pub fn has_atlas(&self) -> bool {
            !self.atlas.is_empty()
        }

        pub fn set_atlas_id(&mut self, id: u32) {
            self.atlas_id = Some(id);
        }
    }
});
