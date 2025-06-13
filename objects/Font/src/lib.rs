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
        kerning_pairs: HashMap<(char, char), i32>,
        // Temporary fields for building glyphs during parsing
        current_chr: Option<char>,
        current_x: Option<u32>,
        current_y: Option<u32>,
        current_w: Option<u32>,
        current_h: Option<u32>,
        current_off_x: Option<i32>,
        current_off_y: Option<i32>,
        current_adv: Option<u32>,
        // Temporary fields for kerning pairs
        in_kerning_array: bool,
        current_left: Option<char>,
        current_right: Option<char>,
        current_kern: Option<i32>,
    }

    impl Font {
        // Visitor pattern methods for JSONLoader
        pub fn visit_start(&mut self) -> Result<(), String> {
            self.glyphs.clear();
            self.kerning_pairs.clear();
            self.in_kerning_array = false;
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
            match key {
                "glyphs" => {
                    self.in_kerning_array = false;
                    true
                }
                "kerning" => {
                    self.in_kerning_array = true;
                    true
                }
                _ => false,
            }
        }

        pub fn visit_object_start(&mut self) -> Result<(), String> {
            if self.in_kerning_array {
                // Clear temporary kerning fields
                self.current_left = None;
                self.current_right = None;
                self.current_kern = None;
            } else {
                // Clear temporary glyph fields
                self.current_chr = None;
                self.current_x = None;
                self.current_y = None;
                self.current_w = None;
                self.current_h = None;
                self.current_off_x = None;
                self.current_off_y = None;
                self.current_adv = None;
            }
            Ok(())
        }

        pub fn visit_object_field(&mut self, key: &str, value: &str) -> Result<(), String> {
            if self.in_kerning_array {
                match key {
                    "left" => self.current_left = value.chars().next(),
                    "right" => self.current_right = value.chars().next(),
                    "kern" => self.current_kern = Some(value.parse().map_err(|_| "invalid kern")?),
                    _ => {}
                }
            } else {
                match key {
                    "chr" => self.current_chr = value.chars().next(),
                    "x" => self.current_x = Some(value.parse().map_err(|_| "invalid x")?),
                    "y" => self.current_y = Some(value.parse().map_err(|_| "invalid y")?),
                    "w" => self.current_w = Some(value.parse().map_err(|_| "invalid w")?),
                    "h" => self.current_h = Some(value.parse().map_err(|_| "invalid h")?),
                    "off_x" => self.current_off_x = Some(value.parse().map_err(|_| "invalid off_x")?),
                    "off_y" => self.current_off_y = Some(value.parse().map_err(|_| "invalid off_y")?),
                    "adv" => self.current_adv = Some(value.parse().map_err(|_| "invalid adv")?),
                    _ => {}
                }
            }
            Ok(())
        }

        pub fn visit_object_end(&mut self) -> Result<(), String> {
            if self.in_kerning_array {
                // Build and insert kerning pair if we have all required fields
                if let (Some(left), Some(right), Some(kern)) =
                    (self.current_left, self.current_right, self.current_kern)
                {
                    self.kerning_pairs.insert((left, right), kern);
                }
            } else {
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
                    self.glyphs.insert(
                        chr,
                        Glyph { x, y, width: w, height: h, offset_x: off_x, offset_y: off_y, advance: adv },
                    );
                }
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
        pub fn glyph(&self, chr: char) -> Option<(u32, u32, u32, u32, i32, i32, u32)> {
            self.glyphs.get(&chr).map(|g| (g.x, g.y, g.width, g.height, g.offset_x, g.offset_y, g.advance))
        }

        pub fn has_glyph(&mut self, chr: char) -> bool {
            self.glyphs.contains_key(&chr)
        }

        pub fn kerning(&self, left: char, right: char) -> i32 {
            self.kerning_pairs.get(&(left, right)).copied().unwrap_or(0)
        }
    }
});
