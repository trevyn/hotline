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
            self.highlight_lens = None;
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
            // First check for hits
            let mut hit_index = None;
            let mut hit_position = (0.0, 0.0);

            for (i, rect_handle) in self.rects.iter().enumerate().rev() {
                // Check if this rect contains the point using dynamic dispatch
                if let Some(contains) = with_library_registry(|registry| {
                    if let Ok(mut rect_guard) = rect_handle.lock() {
                        let rect_any = rect_guard.as_any_mut();
                        let symbol_name = format!(
                            "Rect__contains_point______obj_mut_dyn_Any____point_x__f64____point_y__f64____to__bool__{}",
                            hotline::RUSTC_COMMIT
                        );

                        type ContainsFn =
                            unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64) -> bool;
                        registry
                            .with_symbol::<ContainsFn, _, _>(
                                "librect",
                                &symbol_name,
                                |contains_fn| unsafe { (**contains_fn)(rect_any, x, y) },
                            )
                            .unwrap_or(false)
                    } else {
                        false
                    }
                }) {
                    if contains {
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

                // Create HighlightLens for selected rect
                if let Some(highlight_lens) = with_library_registry(|registry| {
                    if let Ok(hl_obj) = registry.call_constructor("libHighlightLens", "HighlightLens", hotline::RUSTC_COMMIT) {
                        let hl_handle = Arc::new(Mutex::new(hl_obj));
                        
                        // Set the target to the selected rect
                        if let Ok(mut hl_guard) = hl_handle.lock() {
                            let hl_any = hl_guard.as_any_mut();
                            let set_target_symbol = format!("HighlightLens__set_target______obj_mut_dyn_Any____target__ObjectHandle____to__unit__{}", hotline::RUSTC_COMMIT);
                            
                            type SetTargetFn = unsafe extern "Rust" fn(&mut dyn std::any::Any, ObjectHandle);
                            let _ = registry.with_symbol::<SetTargetFn, _, _>("libHighlightLens", &set_target_symbol, |set_fn| {
                                unsafe { (**set_fn)(hl_any, rect_handle.clone()) };
                            });
                        }
                        
                        Some(hl_handle)
                    } else {
                        None
                    }
                }).flatten() {
                    self.highlight_lens = Some(highlight_lens);
                }
            } else {
                // No hit - start rect creation
                self.drag_start = Some((x, y));
            }
        }

        pub fn handle_mouse_up(&mut self, x: f64, y: f64) {
            if self.dragging {
                self.stop_dragging();
            } else if let Some((start_x, start_y)) = self.drag_start {
                // Create a new rect directly
                let width = (x - start_x).abs();
                let height = (y - start_y).abs();
                let rect_x = start_x.min(x);
                let rect_y = start_y.min(y);

                // Create rect via registry
                let new_rect = with_library_registry(|registry| {
                    if let Ok(rect_obj) = registry.call_constructor("librect", "Rect", hotline::RUSTC_COMMIT) {
                        let rect_handle = Arc::new(Mutex::new(rect_obj));
                        // Initialize the rect with position and size
                        if let Ok(mut rect_guard) = rect_handle.lock() {
                            let rect_any = rect_guard.as_any_mut();
                            
                            // Initialize rect
                            let init_symbol = format!("Rect__initialize______obj_mut_dyn_Any____x__f64____y__f64____width__f64____height__f64____to__unit__{}", hotline::RUSTC_COMMIT);
                            type InitFn = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64, f64, f64);
                            let _ = registry.with_symbol::<InitFn, _, _>("librect", &init_symbol, |init_fn| {
                                unsafe { (**init_fn)(rect_any, rect_x, rect_y, width, height) };
                            });
                        }
                        Some(rect_handle)
                    } else {
                        None
                    }
                }).flatten();

                if let Some(rect_handle) = new_rect {
                    // Add to our rects list
                    self.rects.push(rect_handle);
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

        pub fn render(
            &mut self,
            buffer: &mut [u8],
            buffer_width: i64,
            buffer_height: i64,
            pitch: i64,
        ) {
            // Render all rects
            for rect_handle in &self.rects {
                if let Ok(mut rect_guard) = rect_handle.lock() {
                    let rect_any = rect_guard.as_any_mut();

                    // Call rect's render method
                    with_library_registry(|registry| {
                        let render_symbol = format!(
                            "Rect__render______obj_mut_dyn_Any____buffer__mut_ref_slice_u8____buffer_width__i64____buffer_height__i64____pitch__i64____to__unit__{}",
                            hotline::RUSTC_COMMIT
                        );

                        type RenderFn = unsafe extern "Rust" fn(
                            &mut dyn std::any::Any,
                            &mut [u8],
                            i64,
                            i64,
                            i64,
                        );
                        let _ = registry.with_symbol::<RenderFn, _, _>(
                            "librect",
                            &render_symbol,
                            |render_fn| {
                                unsafe {
                                    (**render_fn)(
                                        rect_any,
                                        buffer,
                                        buffer_width,
                                        buffer_height,
                                        pitch,
                                    )
                                };
                            },
                        );
                    });
                }
            }

            // Render the highlight lens if we have one (this will render the selected rect with highlight)
            if let Some(ref hl_handle) = self.highlight_lens {
                if let Ok(mut hl_guard) = hl_handle.lock() {
                    let hl_any = hl_guard.as_any_mut();

                    with_library_registry(|registry| {
                        let render_symbol = format!(
                            "HighlightLens__render______obj_mut_dyn_Any____buffer__mut_ref_slice_u8____buffer_width__i64____buffer_height__i64____pitch__i64____to__unit__{}",
                            hotline::RUSTC_COMMIT
                        );

                        type RenderFn = unsafe extern "Rust" fn(
                            &mut dyn std::any::Any,
                            &mut [u8],
                            i64,
                            i64,
                            i64,
                        );
                        let _ = registry.with_symbol::<RenderFn, _, _>(
                            "libHighlightLens",
                            &render_symbol,
                            |render_fn| {
                                unsafe {
                                    (**render_fn)(
                                        hl_any,
                                        buffer,
                                        buffer_width,
                                        buffer_height,
                                        pitch,
                                    )
                                };
                            },
                        );
                    });
                }
            }
        }
    }
});
