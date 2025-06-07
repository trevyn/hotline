hotline::object!({
    #[derive(Default, Clone)]
    pub struct Circle {
        x: f64,
        y: f64,
        radius: f64,
    }

    impl Circle {
        pub fn initialize(&mut self, x: f64, y: f64, radius: f64) {
            self.x = x;
            self.y = y;
            self.radius = radius;
        }

        pub fn contains_point(&mut self, point_x: f64, point_y: f64) -> bool {
            let dx = point_x - self.x;
            let dy = point_y - self.y;
            dx * dx + dy * dy <= self.radius * self.radius
        }

        pub fn position(&mut self) -> (f64, f64) {
            (self.x, self.y)
        }

        pub fn radius(&mut self) -> f64 {
            self.radius
        }

        pub fn bounds(&mut self) -> (f64, f64, f64, f64) {
            (self.x - self.radius, self.y - self.radius, self.radius * 2.0, self.radius * 2.0)
        }

        pub fn corners(&mut self) -> [(f64, f64); 4] {
            let (x, y, w, h) = self.bounds();
            [(x, y), (x + w, y), (x + w, y + h), (x, y + h)]
        }

        pub fn move_by(&mut self, dx: f64, dy: f64) {
            self.x += dx;
            self.y += dy;
        }

        pub fn resize(&mut self, x: f64, y: f64, width: f64, height: f64) {
            let r = (width.max(height)) / 2.0;
            self.x = x + width / 2.0;
            self.y = y + height / 2.0;
            self.radius = r.max(1.0);
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            let x_start = ((self.x - self.radius) as i32).max(0) as u32;
            let y_start = ((self.y - self.radius) as i32).max(0) as u32;
            let x_end = ((self.x + self.radius) as i32).min(buffer_width as i32) as u32;
            let y_end = ((self.y + self.radius) as i32).min(buffer_height as i32) as u32;

            for y in y_start..y_end {
                for x in x_start..x_end {
                    let dx = x as f64 - self.x;
                    let dy = y as f64 - self.y;
                    if dx * dx + dy * dy <= self.radius * self.radius {
                        let offset = (y * (pitch as u32) + x * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = 0; // B
                            buffer[offset + 1] = 120; // G
                            buffer[offset + 2] = 0; // R
                            buffer[offset + 3] = 255; // A
                        }
                    }
                }
            }
        }
    }
});
