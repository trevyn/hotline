use hotline::object;

object!({
    #[derive(Default, Clone)]
    pub struct Rect {
        pub x: f64,
        pub y: f64,
        pub width: f64,
        pub height: f64,
    }

    impl Rect {
        fn move_by(&mut self, dx: f64, dy: f64) {
            self.x += dx;
            self.y += dy;
        }
        fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            // Draw rectangle by setting pixels
            let x_start = (self.x as i32).max(0) as u32;
            let y_start = (self.y as i32).max(0) as u32;
            let x_end = ((self.x + self.width) as i32).min(buffer_width as i32) as u32;
            let y_end = ((self.y + self.height) as i32).min(buffer_height as i32) as u32;

            for y in y_start..y_end {
                for x in x_start..x_end {
                    let offset = (y * (pitch as u32) + x * 4) as usize;
                    if offset + 3 < buffer.len() {
                        buffer[offset] = 120; // B
                        buffer[offset + 1] = 120; // G
                        buffer[offset + 2] = 0; // R
                        buffer[offset + 3] = 255; // A
                    }
                }
            }
        }
    }
});
