hotline::object!({
    #[derive(Default, Clone)]
    pub struct Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }

    impl Rect {
        pub fn initialize(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.x = x;
            self.y = y;
            self.width = width;
            self.height = height;
        }

        pub fn contains_point(&mut self, point_x: f64, point_y: f64) -> bool {
            point_x >= self.x && point_x <= self.x + self.width && point_y >= self.y && point_y <= self.y + self.height
        }

        pub fn position(&mut self) -> (f64, f64) {
            (self.x, self.y)
        }

        pub fn bounds(&mut self) -> (f64, f64, f64, f64) {
            (self.x, self.y, self.width, self.height)
        }

        pub fn move_by(&mut self, dx: f64, dy: f64) {
            self.x += dx;
            self.y += dy;
        }

        pub fn resize(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.x = x;
            self.y = y;
            self.width = width;
            self.height = height;
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            // Draw rectangle by setting pixels
            let x_start = (self.x as i32).max(0) as u32;
            let y_start = (self.y as i32).max(0) as u32;
            let x_end = ((self.x + self.width) as i32).min(buffer_width as i32) as u32;
            let y_end = ((self.y + self.height) as i32).min(buffer_height as i32) as u32;

            // Draw filled rectangle
            for y in y_start..y_end {
                for x in x_start..x_end {
                    let offset = (y * (pitch as u32) + x * 4) as usize;
                    if offset + 3 < buffer.len() {
                        buffer[offset] = 120; // B
                        buffer[offset + 1] = 0; // G
                        buffer[offset + 2] = 0; // R
                        buffer[offset + 3] = 255; // A
                    }
                }
            }
        }
    }
});
