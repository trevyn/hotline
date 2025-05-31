use hotline::{Renderable, Value, object};

object! {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }

    method moveBy x: f64, y: y f64 |slf| {
        slf.x += x;
        slf.y += y;
        Value::Nil
    }
}

impl Renderable for Rect {
    fn render_to_buffer(&self, buffer: &mut [u8], width: i64, height: i64, pitch: i64) {
        // Draw rectangle by setting pixels
        let x_start = (self.x as i32).max(0) as u32;
        let y_start = (self.y as i32).max(0) as u32;
        let x_end = ((self.x + self.width) as i32).min(width as i32) as u32;
        let y_end = ((self.y + self.height) as i32).min(height as i32) as u32;

        for y in y_start..y_end {
            for x in x_start..x_end {
                let offset = (y * (pitch as u32) + x * 4) as usize;
                if offset + 3 < buffer.len() {
                    buffer[offset] = 255; // B
                    buffer[offset + 1] = 0; // G
                    buffer[offset + 2] = 0; // R
                    buffer[offset + 3] = 255; // A
                }
            }
        }
    }
}
