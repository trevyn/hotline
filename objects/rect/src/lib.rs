use hotline::{Value, object};

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

    method renderToBuffer buf_ptr: *mut u8, width: w i64, height: h i64, pitch: p i64 |slf| {
        eprintln!("renderToBuffer called for rect at ({}, {}) size {}x{}", slf.x, slf.y, slf.width, slf.height);
        let buffer = unsafe {
            std::slice::from_raw_parts_mut(
                buf_ptr,
                (h * p) as usize,
            )
        };

        // Draw rectangle by setting pixels
        let x_start = (slf.x as i32).max(0) as u32;
        let y_start = (slf.y as i32).max(0) as u32;
        let x_end = ((slf.x + slf.width) as i32).min(w as i32) as u32;
        let y_end = ((slf.y + slf.height) as i32).min(h as i32) as u32;

        eprintln!("Drawing from ({}, {}) to ({}, {})", x_start, y_start, x_end, y_end);

        for y in y_start..y_end {
            for x in x_start..x_end {
                let offset = (y * (p as u32) + x * 4) as usize;
                if offset + 3 < buffer.len() {
                    buffer[offset] = 255; // B
                    buffer[offset + 1] = 0; // G
                    buffer[offset + 2] = 255; // R
                    buffer[offset + 3] = 255; // A
                }
            }
        }
        Value::Nil
    }
}
