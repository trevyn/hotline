use hotline::{ObjectHandle, object};
use std::sync::{Arc, Mutex};

object!({
    #[derive(Default)]
    pub struct WindowManager {
        rects: Vec<ObjectHandle>,
        selected: Option<ObjectHandle>,
        highlight_lens: Option<ObjectHandle>, // HighlightLens for selected rect
        dragging: bool,
        drag_offset_x: f64,
        drag_offset_y: f64,
        drag_start: Option<(f64, f64)>,
    }

    impl WindowManager {
        pub fn add_rect(&mut self, rect: ObjectHandle) {
            self.rects.push(rect);
        }

        pub fn clear_selection(&mut self) {
            self.selected = None;
            self.dragging = false;
        }

        pub fn set_drag_offset(&mut self, x: f64, y: f64) {
            self.drag_offset_x = x;
            self.drag_offset_y = y;
        }

        pub fn start_dragging(&mut self, rect: ObjectHandle) {
            self.selected = Some(rect);
            self.dragging = true;
        }

        pub fn stop_dragging(&mut self) {
            self.dragging = false;
        }

        pub fn get_selected_handle(&mut self) -> Option<ObjectHandle> {
            self.selected.clone()
        }

        pub fn get_rects_count(&mut self) -> i64 {
            self.rects.len() as i64
        }

        pub fn get_rect_at(&mut self, index: i64) -> Option<ObjectHandle> {
            if index >= 0 && (index as usize) < self.rects.len() {
                Some(self.rects[index as usize].clone())
            } else {
                None
            }
        }

        pub fn is_dragging(&mut self) -> bool {
            self.dragging
        }
        

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) {
            println!("WindowManager::handle_mouse_down({}, {})", x, y);
            
            // First check for hits
            let mut hit_index = None;
            let mut hit_position = (0.0, 0.0);
            
            for (i, rect_handle) in self.rects.iter().enumerate().rev() {
                // Check if this rect contains the point using dynamic dispatch
                if let Some(contains) = with_library_registry(|registry| {
                    if let Ok(mut rect_guard) = rect_handle.lock() {
                        let rect_any = rect_guard.as_any_mut();
                        let symbol_name = format!("Rect__contains_point______obj_mut_dyn_Any____point_x__f64____point_y__f64____to__bool__{}", hotline::RUSTC_COMMIT);
                        
                        type ContainsFn = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64) -> bool;
                        registry.with_symbol::<ContainsFn, _, _>("librect", &symbol_name, |contains_fn| {
                            unsafe { (**contains_fn)(rect_any, x, y) }
                        }).unwrap_or(false)
                    } else {
                        false
                    }
                }) {
                    if contains {
                        println!("Hit rect at index {}", i);
                        hit_index = Some(i);
                        
                        // Get rect position for offset calculation
                        if let Ok(mut rect_guard) = rect_handle.lock() {
                            let rect_any = rect_guard.as_any_mut();
                            if let Some((rx, ry)) = with_library_registry(|registry| {
                                let symbol_name = format!("Rect__position______obj_mut_dyn_Any____to__tuple_f64_comma_f64__{}", hotline::RUSTC_COMMIT);
                                
                                type PositionFn = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> (f64, f64);
                                registry.with_symbol::<PositionFn, _, _>("librect", &symbol_name, |pos_fn| {
                                    unsafe { (**pos_fn)(rect_any) }
                                }).ok()
                            }).flatten() {
                                hit_position = (rx, ry);
                            }
                        }
                        break;
                    }
                }
            }
            
            // Clear previous selection
            self.clear_selection();
            
            if let Some(index) = hit_index {
                // Found a hit - select it
                let rect_handle = &self.rects[index];
                
                // Calculate drag offset from click position to rect position
                self.drag_offset_x = hit_position.0 - x;
                self.drag_offset_y = hit_position.1 - y;
                self.selected = Some(rect_handle.clone());
                self.dragging = true;
                println!("Started dragging rect with offset ({}, {})", self.drag_offset_x, self.drag_offset_y);
            } else {
                // No hit - start rect creation
                self.drag_start = Some((x, y));
                println!("Starting rect creation at ({}, {})", x, y);
            }
        }
        
        pub fn handle_mouse_up(&mut self, x: f64, y: f64) {
            println!("WindowManager::handle_mouse_up({}, {})", x, y);
            
            if self.dragging {
                self.stop_dragging();
                println!("Stopped dragging");
            } else if let Some((start_x, start_y)) = self.drag_start {
                // Create a new rect directly
                let width = (x - start_x).abs();
                let height = (y - start_y).abs();
                let rect_x = start_x.min(x);
                let rect_y = start_y.min(y);
                
                // Create rect via registry
                println!("About to call with_library_registry...");
                let new_rect = with_library_registry(|registry| {
                    println!("Inside with_library_registry callback");
                    println!("Calling registry.call_constructor...");
                    if let Ok(rect_obj) = registry.call_constructor("librect", "Rect", hotline::RUSTC_COMMIT) {
                        println!("Successfully created rect object");
                        let rect_handle = Arc::new(Mutex::new(rect_obj));
                        // Initialize the rect with position and size
                        println!("Attempting to lock rect_handle...");
                        if let Ok(mut rect_guard) = rect_handle.lock() {
                            println!("Successfully locked rect_handle");
                            let rect_any = rect_guard.as_any_mut();
                            
                            // Initialize rect
                            let init_symbol = format!("Rect__initialize______obj_mut_dyn_Any____x__f64____y__f64____width__f64____height__f64____to__unit__{}", hotline::RUSTC_COMMIT);
                            type InitFn = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64, f64, f64);
                            println!("Looking up init symbol: {}", init_symbol);
                            let result = registry.with_symbol::<InitFn, _, _>("librect", &init_symbol, |init_fn| {
                                println!("Found init symbol, calling it...");
                                unsafe { (**init_fn)(rect_any, rect_x, rect_y, width, height) };
                                println!("Init symbol called successfully");
                            });
                            if let Err(e) = result {
                                println!("Failed to call init symbol: {:?}", e);
                            }
                        } else {
                            println!("Failed to lock rect_handle");
                        }
                        Some(rect_handle)
                    } else {
                        println!("Failed to create rect object");
                        None
                    }
                });
                
                if new_rect.is_none() {
                    println!("WARNING: with_library_registry returned None - library registry not initialized!");
                } else {
                    println!("with_library_registry returned Some");
                }
                
                let new_rect = new_rect.flatten();
                
                if let Some(rect_handle) = new_rect {
                    // Add to our rects list
                    self.rects.push(rect_handle);
                    println!("Created rect at ({}, {}) with size ({}, {})", rect_x, rect_y, width, height);
                }
                
                self.drag_start = None;
            }
        }
        
        pub fn handle_mouse_motion(&mut self, x: f64, y: f64) {
            if self.dragging {
                if let Some(ref selected_handle) = self.selected {
                    // Move the selected rect to follow the mouse
                    let new_x = x + self.drag_offset_x;
                    let new_y = y + self.drag_offset_y;
                    
                    if let Ok(mut rect_guard) = selected_handle.lock() {
                        let rect_any = rect_guard.as_any_mut();
                        
                        // Get current position
                        if let Some((current_x, current_y)) = with_library_registry(|registry| {
                            let pos_symbol = format!("Rect__position______obj_mut_dyn_Any____to__tuple_f64_comma_f64__{}", hotline::RUSTC_COMMIT);
                            
                            type PositionFn = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> (f64, f64);
                            registry.with_symbol::<PositionFn, _, _>("librect", &pos_symbol, |pos_fn| {
                                unsafe { (**pos_fn)(rect_any) }
                            }).ok()
                        }).flatten() {
                            // Calculate delta movement
                            let dx = new_x - current_x;
                            let dy = new_y - current_y;
                            
                            // Move the rect
                            with_library_registry(|registry| {
                                let move_symbol = format!("Rect__move_by______obj_mut_dyn_Any____dx__f64____dy__f64____to__unit__{}", hotline::RUSTC_COMMIT);
                                
                                type MoveFn = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64);
                                let _ = registry.with_symbol::<MoveFn, _, _>("librect", &move_symbol, |move_fn| {
                                    unsafe { (**move_fn)(rect_any, dx, dy) };
                                });
                            });
                        }
                    }
                }
            }
        }
        
        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            // Render all rects
            for rect_handle in &self.rects {
                if let Ok(mut rect_guard) = rect_handle.lock() {
                    let rect_any = rect_guard.as_any_mut();
                    
                    // Call rect's render method
                    with_library_registry(|registry| {
                        let render_symbol = format!("Rect__render______obj_mut_dyn_Any____buffer__mut_ref_slice_u8____buffer_width__i64____buffer_height__i64____pitch__i64____to__unit__{}", hotline::RUSTC_COMMIT);
                        
                        type RenderFn = unsafe extern "Rust" fn(&mut dyn std::any::Any, &mut [u8], i64, i64, i64);
                        let _ = registry.with_symbol::<RenderFn, _, _>("librect", &render_symbol, |render_fn| {
                            unsafe { (**render_fn)(rect_any, buffer, buffer_width, buffer_height, pitch) };
                        });
                    });
                }
            }
        }
    }
});
