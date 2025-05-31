use hotline::{Serialize, Value, object};

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

    method bounds |slf| {
        let bounds = hotline::Bounds::new(slf.x, slf.y, slf.width, slf.height);
        <hotline::Bounds as Serialize>::serialize(&bounds)
    }

    method properties |slf| {
        hotline::dict! {
            "x" => Value::Float(slf.x),
            "y" => Value::Float(slf.y),
            "width" => Value::Float(slf.width),
            "height" => Value::Float(slf.height)
        }
    }

}

// Simple standalone render function with Rust signature
#[unsafe(no_mangle)]
pub extern "C" fn render_rect(
    obj: &dyn std::any::Any,
    buffer: &mut [u8],
    buffer_width: i64,
    buffer_height: i64,
    pitch: i64,
) {
    // Downcast to Rect
    let Some(rect) = obj.downcast_ref::<Rect>() else {
        return;
    };

    // Draw rectangle by setting pixels
    let x_start = (rect.x as i32).max(0) as u32;
    let y_start = (rect.y as i32).max(0) as u32;
    let x_end = ((rect.x + rect.width) as i32).min(buffer_width as i32) as u32;
    let y_end = ((rect.y + rect.height) as i32).min(buffer_height as i32) as u32;

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
