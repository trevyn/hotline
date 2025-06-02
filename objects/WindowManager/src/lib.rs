use hotline::{ObjectHandle, object, HotlineObject};

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
            // First check for hits
            let mut hit_index = None;
            let mut hit_position = (0.0, 0.0);
            
            for (i, rect_handle) in self.rects.iter().enumerate().rev() {
                // Try to lock and check if this rect contains the point
                if let Ok(mut rect_guard) = rect_handle.lock() {
                    // We need to dynamically check and call methods via the object interface
                    // For now, just track which rect was clicked
                    // TODO: Need runtime to provide method calling capabilities
                    hit_index = Some(i);
                    break;
                }
            }
            
            // Clear previous selection
            self.clear_selection();
            
            if let Some(index) = hit_index {
                // Found a hit - select it
                let rect_handle = &self.rects[index];
                
                // Set up dragging with default offset for now
                self.drag_offset_x = 10.0;
                self.drag_offset_y = 10.0;
                self.selected = Some(rect_handle.clone());
                self.dragging = true;
            } else {
                // No hit - start rect creation
                self.drag_start = Some((x, y));
            }
        }
        
        pub fn handle_mouse_up(&mut self, x: f64, y: f64) {
            if self.dragging {
                self.stop_dragging();
            } else if let Some((start_x, start_y)) = self.drag_start {
                // TODO: Need runtime to create new rect instances
                // For now just clear the drag_start
                self.drag_start = None;
            }
        }
        
        pub fn handle_mouse_motion(&mut self, x: f64, y: f64) {
            if self.dragging {
                if let Some(ref selected_handle) = self.selected {
                    // TODO: Need runtime to call move_by on the rect
                    // For now just track the motion
                }
            }
        }
        
        pub fn render(&mut self, buffer: &mut [u8], _buffer_width: i64, _buffer_height: i64, _pitch: i64) {
            // TODO: Need runtime to call render on each rect
            // For now, buffer is already cleared by the caller
        }
    }
});
