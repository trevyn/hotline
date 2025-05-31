use hotline::{MethodSignature, TypedMessage, TypedObject, TypedValue, typed_methods};

#[derive(Clone)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

typed_methods! {
    Rect {
        fn move_by(&mut self, dx: f64, dy: f64) {
            self.x += dx;
            self.y += dy;
        }

        fn get_x(&mut self) -> f64 {
            self.x
        }

        fn get_y(&mut self) -> f64 {
            self.y
        }

        fn get_width(&mut self) -> f64 {
            self.width
        }

        fn get_height(&mut self) -> f64 {
            self.height
        }

        fn set_x(&mut self, x: f64) {
            self.x = x;
        }

        fn set_y(&mut self, y: f64) {
            self.y = y;
        }

        fn set_width(&mut self, width: f64) {
            self.width = width;
        }

        fn set_height(&mut self, height: f64) {
            self.height = height;
        }
    }
}

// Constructor for hot-reloading - uses Rust ABI to preserve trait object
#[unsafe(no_mangle)]
pub extern "Rust" fn create_rect() -> Box<dyn TypedObject> {
    Box::new(Rect { x: 100.0, y: 100.0, width: 200.0, height: 150.0 })
}

// Simple standalone render function with Rust signature
#[unsafe(no_mangle)]
pub extern "Rust" fn render_rect(
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
                buffer[offset + 1] = 120; // G
                buffer[offset + 2] = 0; // R
                buffer[offset + 3] = 255; // A
            }
        }
    }
}
