use std::sync::{Arc, Mutex};

hotline::object!({
    #[derive(Default)]
    pub struct WindowManager {
        rects: Vec<Rect>,
        selected: Option<Rect>,
        highlight_lens: Option<HighlightLens>, // HighlightLens for selected rect
        dragging: bool,
        drag_offset_x: f64,
        drag_offset_y: f64,
        drag_start: Option<(f64, f64)>,
    }

    impl WindowManager {
        pub fn add_rect(&mut self, rect: Rect) {
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

        pub fn start_dragging(&mut self, rect: Rect) {
            self.selected = Some(rect);
            self.dragging = true;
        }

        pub fn stop_dragging(&mut self) {
            self.dragging = false;
        }

        pub fn get_selected_handle(&mut self) -> Option<Rect> {
            self.selected.clone()
        }

        pub fn get_rects_count(&mut self) -> i64 {
            self.rects.len() as i64
        }

        pub fn get_rect_at(&mut self, index: i64) -> Option<Rect> {
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

            for (i, rect_handle) in self.rects.iter_mut().enumerate().rev() {
                // Check if this rect contains the point
                if rect_handle.contains_point(x, y) {
                    hit_index = Some(i);
                    
                    // Get rect position for offset calculation
                    hit_position = rect_handle.position();
                    break;
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
                if let Some(hl_obj) = crate::with_library_registry(|registry| {
                    registry.call_constructor("libHighlightLens", "HighlightLens", hotline::RUSTC_COMMIT).ok()
                }).flatten() {
                    let mut hl_handle = HighlightLens::from_handle(Arc::new(Mutex::new(hl_obj)));
                    
                    // Set the target to the selected rect  
                    hl_handle.set_target(rect_handle.handle().clone());
                    
                    self.highlight_lens = Some(hl_handle);
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
                let new_rect = crate::with_library_registry(|registry| {
                    if let Ok(rect_obj) = registry.call_constructor("libRect", "Rect", hotline::RUSTC_COMMIT) {
                        let mut rect_handle = Rect::from_handle(Arc::new(Mutex::new(rect_obj)));
                        
                        // Initialize the rect with position and size
                        rect_handle.initialize(rect_x, rect_y, width, height);
                        
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
                if let Some(ref mut selected_handle) = self.selected {
                    // Move the selected rect to follow the mouse
                    let new_x = x + self.drag_offset_x;
                    let new_y = y + self.drag_offset_y;

                    // Get current position
                    let (current_x, current_y) = selected_handle.position();
                    
                    // Calculate delta movement
                    let dx = new_x - current_x;
                    let dy = new_y - current_y;
                    
                    // Move the rect
                    selected_handle.move_by(dx, dy);
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
            for rect_handle in &mut self.rects {
                rect_handle.render(buffer, buffer_width, buffer_height, pitch);
            }

            // Render the highlight lens if we have one (this will render the selected rect with highlight)
            if let Some(ref mut hl_handle) = self.highlight_lens {
                hl_handle.render(buffer, buffer_width, buffer_height, pitch);
            }
        }
    }
});
