use std::collections::HashMap;

hotline::object!({
    #[derive(Clone, Default)]
    struct Glyph {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        offset_x: i32,
        offset_y: i32,
        advance: u32,
    }

    #[derive(Clone, Default)]
    pub struct Font {
        pub size: u32,
        pub line_gap: u32,
        pub space_width: u32,
        glyphs: HashMap<char, Glyph>,
        // Temporary fields for building glyphs during parsing
        current_chr: Option<char>,
        current_x: Option<u32>,
        current_y: Option<u32>,
        current_w: Option<u32>,
        current_h: Option<u32>,
        current_off_x: Option<i32>,
        current_off_y: Option<i32>,
        current_adv: Option<u32>,
    }

    impl Font {
        // Visitor pattern methods for JSONLoader
        pub fn visit_start(&mut self) -> Result<(), String> {
            self.glyphs.clear();
            Ok(())
        }

        pub fn visit_field(&mut self, key: &str, value: &str) -> Result<(), String> {
            match key {
                "size" => self.size = value.parse().map_err(|_| "invalid size")?,
                "line_gap" => self.line_gap = value.parse().map_err(|_| "invalid line_gap")?,
                "space_w" => self.space_width = value.parse().map_err(|_| "invalid space_w")?,
                _ => {} // ignore unknown fields
            }
            Ok(())
        }

        pub fn visit_array_start(&mut self, key: &str) -> bool {
            key == "glyphs"
        }

        pub fn visit_object_start(&mut self) -> Result<(), String> {
            // Clear temporary glyph fields
            self.current_chr = None;
            self.current_x = None;
            self.current_y = None;
            self.current_w = None;
            self.current_h = None;
            self.current_off_x = None;
            self.current_off_y = None;
            self.current_adv = None;
            Ok(())
        }

        pub fn visit_object_field(&mut self, key: &str, value: &str) -> Result<(), String> {
            match key {
                "chr" => self.current_chr = value.chars().next(),
                "x" => self.current_x = value.parse().ok(),
                "y" => self.current_y = value.parse().ok(),
                "w" => self.current_w = value.parse().ok(),
                "h" => self.current_h = value.parse().ok(),
                "off_x" => self.current_off_x = value.parse().ok(),
                "off_y" => self.current_off_y = value.parse().ok(),
                "adv" => self.current_adv = value.parse().ok(),
                _ => {}
            }
            Ok(())
        }

        pub fn visit_object_end(&mut self) -> Result<(), String> {
            // Build and insert glyph if we have all required fields
            if let (Some(chr), Some(x), Some(y), Some(w), Some(h), Some(off_x), Some(off_y), Some(adv)) = (
                self.current_chr,
                self.current_x,
                self.current_y,
                self.current_w,
                self.current_h,
                self.current_off_x,
                self.current_off_y,
                self.current_adv,
            ) {
                self.glyphs
                    .insert(chr, Glyph { x, y, width: w, height: h, offset_x: off_x, offset_y: off_y, advance: adv });
            }
            Ok(())
        }

        pub fn visit_array_end(&mut self, _key: &str) -> Result<(), String> {
            Ok(())
        }

        pub fn visit_end(&mut self) -> Result<(), String> {
            Ok(())
        }

        // Font-specific methods
        pub fn glyph(&mut self, chr: char) -> Option<(u32, u32, u32, u32, i32, i32, u32)> {
            self.glyphs.get(&chr).map(|g| (g.x, g.y, g.width, g.height, g.offset_x, g.offset_y, g.advance))
        }

        pub fn has_glyph(&mut self, chr: char) -> bool {
            self.glyphs.contains_key(&chr)
        }
    }
});
