use hotline::{object, ObjectHandle};

object!({
    #[derive(Default)]
    pub struct HighlightLens {
        target: Option<ObjectHandle>,
        highlight_color: (u8, u8, u8, u8), // BGRA
    }

    impl HighlightLens {
        pub fn set_target(&mut self, target: ObjectHandle) {
            self.target = Some(target);
            self.highlight_color = (0, 255, 0, 255); // Green by default
        }

        pub fn set_highlight_color(&mut self, b: u8, g: u8, r: u8, a: u8) {
            self.highlight_color = (b, g, r, a);
        }

        pub fn render(
            &mut self,
            buffer: &mut [u8],
            buffer_width: i64,
            buffer_height: i64,
            pitch: i64,
        ) {
            if let Some(ref target) = self.target {
                // Only draw highlight border (rect is already rendered by WindowManager)
                // Dynamically call bounds method on any object that has it
                let (x, y, width, height) = crate::with_library_registry(|registry| {
                    if let Ok(mut guard) = target.lock() {
                        // Get type name first, before mutable borrow
                        let type_name = guard.type_name();
                        
                        // Construct symbol name dynamically based on the object's actual type
                        let symbol_name = format!(
                            "{type_name}__bounds______obj_mut_dyn_Any____to__tuple_f64_comma_f64_comma_f64_comma_f64__5d707b07e"
                        );
                        let lib_name = format!("lib{type_name}");
                        
                        // Now get mutable reference
                        let obj_any = guard.as_any_mut();
                        
                        type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> (f64, f64, f64, f64);
                        registry.with_symbol::<FnType, _, _>(
                            &lib_name,
                            &symbol_name,
                            |fn_ptr| unsafe { (**fn_ptr)(obj_any) }
                        ).unwrap_or_else(|e| panic!("Target object {} doesn't have bounds method: {}", type_name, e))
                    } else {
                        panic!("Failed to lock target object")
                    }
                }).unwrap_or_else(|| panic!("No library registry available"));

                let x_start = (x as i32).max(0) as u32;
                let y_start = (y as i32).max(0) as u32;
                let x_end = ((x + width) as i32).min(buffer_width as i32) as u32;
                let y_end = ((y + height) as i32).min(buffer_height as i32) as u32;

                let (b, g, r, a) = self.highlight_color;

                // Top and bottom borders
                for x in x_start..x_end {
                    let top_offset = (y_start * (pitch as u32) + x * 4) as usize;
                    let bottom_offset = ((y_end - 1) * (pitch as u32) + x * 4) as usize;
                    if top_offset + 3 < buffer.len() {
                        buffer[top_offset] = b;
                        buffer[top_offset + 1] = g;
                        buffer[top_offset + 2] = r;
                        buffer[top_offset + 3] = a;
                    }
                    if bottom_offset + 3 < buffer.len() {
                        buffer[bottom_offset] = b;
                        buffer[bottom_offset + 1] = g;
                        buffer[bottom_offset + 2] = r;
                        buffer[bottom_offset + 3] = a;
                    }
                }

                // Left and right borders
                for y in y_start..y_end {
                    let left_offset = (y * (pitch as u32) + x_start * 4) as usize;
                    let right_offset = (y * (pitch as u32) + (x_end - 1) * 4) as usize;
                    if left_offset + 3 < buffer.len() {
                        buffer[left_offset] = b;
                        buffer[left_offset + 1] = g;
                        buffer[left_offset + 2] = r;
                        buffer[left_offset + 3] = a;
                    }
                    if right_offset + 3 < buffer.len() {
                        buffer[right_offset] = b;
                        buffer[right_offset + 1] = g;
                        buffer[right_offset + 2] = r;
                        buffer[right_offset + 3] = a;
                    }
                }
            }
        }
    }
});
