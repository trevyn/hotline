hotline::object!({
    use std::collections::HashMap;

    #[derive(Clone, Copy)]
    struct GlyphInfo {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        offset_x: i32,
        offset_y: i32,
        advance: u32,
    }

    #[derive(Clone, Default)]
    pub struct CpuTextRenderer {
        #[default(Vec::new())]
        font_atlas: Vec<u8>, // RGBA bitmap
        #[default(0)]
        atlas_width: u32,
        #[default(0)]
        atlas_height: u32,
        #[default(HashMap::new())]
        glyphs: HashMap<char, GlyphInfo>,
        #[default(HashMap::new())]
        kerning: HashMap<(char, char), i32>,
        #[default(0)]
        font_size: u32,
        #[default(0)]
        line_gap: u32,
        #[default(0)]
        space_width: u32,
    }

    impl CpuTextRenderer {
        pub fn initialize(&mut self) {
            // Load font atlas PNG
            let png_data = std::fs::read("fonts/owlet/owlet.png").unwrap();
            let decoder = png::Decoder::new(&png_data[..]);
            let mut reader = decoder.read_info().unwrap();
            let mut atlas_data = vec![0u8; reader.output_buffer_size()];
            reader.next_frame(&mut atlas_data).unwrap();

            // Get actual dimensions from the PNG
            let info = reader.info();
            let atlas_width = info.width;
            let atlas_height = info.height;

            // Convert grayscale-alpha to RGBA
            let mut rgba_atlas = Vec::with_capacity(atlas_data.len() * 2);
            for i in (0..atlas_data.len()).step_by(2) {
                let gray = atlas_data[i];
                let alpha = atlas_data[i + 1];
                rgba_atlas.extend_from_slice(&[gray, gray, gray, alpha]);
            }

            // Load font JSON
            let json_str = std::fs::read_to_string("fonts/owlet/owlet.json").unwrap();
            let json: serde_json::Value = serde_json::from_str(&json_str).unwrap();

            // Parse glyphs
            let mut glyphs = HashMap::new();
            for glyph in json["glyphs"].as_array().unwrap() {
                let ch = glyph["chr"].as_str().unwrap().chars().next().unwrap();
                glyphs.insert(
                    ch,
                    GlyphInfo {
                        x: glyph["x"].as_u64().unwrap() as u32,
                        y: glyph["y"].as_u64().unwrap() as u32,
                        width: glyph["w"].as_u64().unwrap() as u32,
                        height: glyph["h"].as_u64().unwrap() as u32,
                        offset_x: glyph["off_x"].as_i64().unwrap() as i32,
                        offset_y: glyph["off_y"].as_i64().unwrap() as i32,
                        advance: glyph["adv"].as_u64().unwrap() as u32,
                    },
                );
            }

            // Parse kerning pairs
            let mut kerning = HashMap::new();
            if let Some(kerning_array) = json["kerning"].as_array() {
                for k in kerning_array {
                    let first = k["left"].as_str().unwrap().chars().next().unwrap();
                    let second = k["right"].as_str().unwrap().chars().next().unwrap();
                    let amount = k["kern"].as_i64().unwrap() as i32;
                    kerning.insert((first, second), amount);
                }
            }

            self.font_atlas = rgba_atlas;
            self.atlas_width = atlas_width;
            self.atlas_height = atlas_height;
            self.glyphs = glyphs;
            self.kerning = kerning;
            self.font_size = json["size"].as_u64().unwrap() as u32;
            self.line_gap = json["line_gap"].as_u64().unwrap() as u32;
            self.space_width = json["space_w"].as_u64().unwrap() as u32;

            // Debug: log font info
            eprintln!(
                "CpuTextRenderer initialized: atlas={}x{}, font_size={}, glyphs={}",
                atlas_width,
                atlas_height,
                self.font_size,
                self.glyphs.len()
            );
        }

        pub fn render_line(&self, text: String, color: (u8, u8, u8, u8)) -> (Vec<u8>, u32, u32) {
            // Calculate line dimensions by finding the bounding box of all glyphs
            let mut cursor_x = 0i32;
            let mut min_x = 0i32;
            let mut max_x = 0i32;
            let mut prev_char: Option<char> = None;

            for ch in text.chars() {
                if let Some(prev) = prev_char {
                    if let Some(&kern) = self.kerning.get(&(prev, ch)) {
                        cursor_x += kern;
                    }
                }

                if ch == ' ' {
                    cursor_x += self.space_width as i32;
                } else if let Some(glyph) = self.glyphs.get(&ch) {
                    let glyph_left = cursor_x + glyph.offset_x;
                    let glyph_right = glyph_left + glyph.width as i32;
                    min_x = min_x.min(glyph_left);
                    max_x = max_x.max(glyph_right);
                    cursor_x += glyph.advance as i32;
                } else {
                    cursor_x += self.space_width as i32; // fallback
                }
                prev_char = Some(ch);
            }

            // If there was no text, max_x would be 0, but we should make sure it's at least the final cursor position
            max_x = max_x.max(cursor_x);

            let width = (max_x - min_x).max(0) as u32;
            let height = self.font_size + self.line_gap;
            let mut buffer = vec![0u8; (width * height * 4) as usize];

            // Debug: verify buffer size matches our stride calculations
            debug_assert_eq!(buffer.len(), (width * height * 4) as usize);

            // Render glyphs, offsetting by min_x to fit in the buffer
            cursor_x = -min_x;
            prev_char = None;

            for ch in text.chars() {
                // Apply kerning
                if let Some(prev) = prev_char {
                    if let Some(&kern) = self.kerning.get(&(prev, ch)) {
                        cursor_x += kern;
                    }
                }

                if ch == ' ' {
                    cursor_x += self.space_width as i32;
                    prev_char = Some(ch);
                    continue;
                }

                if let Some(glyph) = self.glyphs.get(&ch) {
                    // Validate glyph bounds in atlas
                    if glyph.x + glyph.width > self.atlas_width || glyph.y + glyph.height > self.atlas_height {
                        eprintln!(
                            "WARNING: Glyph '{}' out of atlas bounds: pos=({},{}) size=({},{}) atlas={}x{}",
                            ch, glyph.x, glyph.y, glyph.width, glyph.height, self.atlas_width, self.atlas_height
                        );
                        cursor_x += self.space_width as i32;
                        prev_char = Some(ch);
                        continue;
                    }

                    // Calculate glyph destination bounds
                    let glyph_left = cursor_x + glyph.offset_x;
                    let glyph_right = glyph_left + glyph.width as i32;
                    // Original calculation: offset_y + font_size gives the baseline position
                    let glyph_top = glyph.offset_y + self.font_size as i32;
                    let glyph_bottom = glyph_top + glyph.height as i32;

                    // Check if entire glyph would be out of bounds
                    if glyph_right <= 0 || glyph_left >= width as i32 || glyph_bottom <= 0 || glyph_top >= height as i32
                    {
                        cursor_x += glyph.advance as i32;
                        prev_char = Some(ch);
                        continue;
                    }

                    // Debug log for glyphs that partially exceed bounds
                    if glyph_left < 0 || glyph_right > width as i32 || glyph_top < 0 || glyph_bottom > height as i32 {
                        eprintln!(
                            "DEBUG: Glyph '{}' partially out of bounds: x=[{},{}] y=[{},{}] buffer={}x{}",
                            ch, glyph_left, glyph_right, glyph_top, glyph_bottom, width, height
                        );
                    }

                    // Copy glyph pixels from atlas to buffer with bounds checking
                    for gy in 0..glyph.height {
                        for gx in 0..glyph.width {
                            let dst_x = glyph_left + gx as i32;
                            let dst_y = glyph_top + gy as i32;

                            // Skip pixels outside buffer bounds
                            if dst_x < 0 || dst_x >= width as i32 || dst_y < 0 || dst_y >= height as i32 {
                                continue;
                            }

                            let src_idx = ((glyph.y + gy) * self.atlas_width + (glyph.x + gx)) * 4;
                            let dst_idx = (dst_y as u32 * width + dst_x as u32) * 4;

                            // Double-check indices are valid
                            let src_idx_end = src_idx as usize + 3;
                            let dst_idx_end = dst_idx as usize + 3;
                            if src_idx_end >= self.font_atlas.len() {
                                eprintln!(
                                    "ERROR: Source index out of bounds for '{}': {} >= {}",
                                    ch,
                                    src_idx_end,
                                    self.font_atlas.len()
                                );
                                continue;
                            }
                            if dst_idx_end > buffer.len() {
                                eprintln!(
                                    "ERROR: Dest index out of bounds for '{}': {} > {} (x={}, y={}, width={})",
                                    ch,
                                    dst_idx_end,
                                    buffer.len(),
                                    dst_x,
                                    dst_y,
                                    width
                                );
                                panic!("Buffer overflow detected!");
                            }

                            let alpha = self.font_atlas[src_idx as usize + 3];
                            if alpha > 0 {
                                let idx = dst_idx as usize;
                                buffer[idx] = color.2; // R (from BGR)
                                buffer[idx + 1] = color.1; // G
                                buffer[idx + 2] = color.0; // B
                                buffer[idx + 3] = (alpha as u32 * color.3 as u32 / 255) as u8;
                            }
                        }
                    }
                    cursor_x += glyph.advance as i32;
                } else {
                    // Debug log for missing glyphs
                    if ch != ' ' && ch.is_ascii_graphic() {
                        eprintln!("DEBUG: Missing glyph for character '{}' (U+{:04X})", ch, ch as u32);
                    }
                    cursor_x += self.space_width as i32; // fallback
                }
                prev_char = Some(ch);
            }

            // Verify buffer size is correct
            assert_eq!(buffer.len(), (width * height * 4) as usize);

            (buffer, width, height)
        }
    }
});
