use hotline::{
    Deserialize, Message, Object, Serialize, Value, dict, init_with, object, register_objects,
};

object! {
    Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    }
}

impl Rect {
    fn custom_receive(&mut self, msg: &Message) -> Value {
        match msg.selector.as_str() {
            "render" => {
                if msg.args.len() >= 4 {
                    if let (
                        Value::Int(buf_ptr),
                        Value::Int(width),
                        Value::Int(height),
                        Value::Int(pitch),
                    ) = (&msg.args[0], &msg.args[1], &msg.args[2], &msg.args[3])
                    {
                        let buffer = unsafe {
                            std::slice::from_raw_parts_mut(
                                *buf_ptr as *mut u8,
                                (*height * *pitch) as usize,
                            )
                        };

                        // Draw rectangle by setting pixels
                        let x_start = (self.x as i32).max(0) as u32;
                        let y_start = (self.y as i32).max(0) as u32;
                        let x_end = ((self.x + self.width) as i32).min(*width as i32) as u32;
                        let y_end = ((self.y + self.height) as i32).min(*height as i32) as u32;

                        for y in y_start..y_end {
                            for x in x_start..x_end {
                                let offset = (y * (*pitch as u32) + x * 4) as usize;
                                if offset + 3 < buffer.len() {
                                    buffer[offset] = 255; // B
                                    buffer[offset + 1] = 0; // G
                                    buffer[offset + 2] = 255; // R
                                    buffer[offset + 3] = 255; // A
                                }
                            }
                        }
                    }
                }
                Value::Nil
            }

            "moveBy:y:" => {
                if msg.args.len() >= 2 {
                    if let (Value::Float(dx), Value::Float(dy)) = (&msg.args[0], &msg.args[1]) {
                        self.x += dx;
                        self.y += dy;
                    }
                }
                Value::Nil
            }

            "initWithX:y:width:height:" => init_with!(self, msg, x, y, width, height),

            _ => Value::Nil,
        }
    }
}

register_objects!(Rect);
