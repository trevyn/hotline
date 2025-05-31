use hotline::{Message, Object, Value};

#[derive(Default)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl Object for Rect {
    fn receive(&mut self, msg: &Message) -> Value {
        match msg.selector.as_str() {
            "init" => {
                if let Some(Value::Dict(props)) = msg.args.first() {
                    if let Some(Value::Float(x)) = props.get("x") {
                        self.x = *x;
                    }
                    if let Some(Value::Float(y)) = props.get("y") {
                        self.y = *y;
                    }
                    if let Some(Value::Float(w)) = props.get("width") {
                        self.width = *w;
                    }
                    if let Some(Value::Float(h)) = props.get("height") {
                        self.height = *h;
                    }
                }
                Value::Nil
            }

            "initWithX:y:width:height:" => {
                if msg.args.len() >= 4 {
                    if let Value::Float(x) = &msg.args[0] {
                        self.x = *x;
                    }
                    if let Value::Float(y) = &msg.args[1] {
                        self.y = *y;
                    }
                    if let Value::Float(w) = &msg.args[2] {
                        self.width = *w;
                    }
                    if let Value::Float(h) = &msg.args[3] {
                        self.height = *h;
                    }
                }
                Value::Nil
            }

            "render" => {
                // Expect: buffer pointer, width, height, pitch
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
                                    buffer[offset + 2] = 0; // R
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

            "x" => Value::Float(self.x),
            "y" => Value::Float(self.y),
            "width" => Value::Float(self.width),
            "height" => Value::Float(self.height),

            "x:" => {
                if let Some(Value::Float(x)) = msg.args.first() {
                    self.x = *x;
                }
                Value::Nil
            }

            "y:" => {
                if let Some(Value::Float(y)) = msg.args.first() {
                    self.y = *y;
                }
                Value::Nil
            }

            "width:" => {
                if let Some(Value::Float(w)) = msg.args.first() {
                    self.width = *w;
                }
                Value::Nil
            }

            "height:" => {
                if let Some(Value::Float(h)) = msg.args.first() {
                    self.height = *h;
                }
                Value::Nil
            }

            _ => Value::Nil,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn register_objects(ctx: *mut std::ffi::c_void) {
    let register =
        unsafe { &mut *(ctx as *mut Box<dyn FnMut(&str, Box<dyn Fn() -> Box<dyn Object>>)>) };
    register("Rect", Box::new(|| Box::new(Rect::default())));
}

// For static linking
pub fn register(register: &mut dyn FnMut(&str, Box<dyn Fn() -> Box<dyn Object>>)) {
    register("Rect", Box::new(|| Box::new(Rect::default())));
}
