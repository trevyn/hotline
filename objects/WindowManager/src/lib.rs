use hotline::{object, ObjectHandle};

object!({
    #[derive(Default)]
    pub struct WindowManager {
        pub rects: Vec<ObjectHandle>,
        pub selected: Option<ObjectHandle>,
        pub dragging: bool,
        pub drag_offset_x: f64,
        pub drag_offset_y: f64,
    }

    impl WindowManager {
        fn add_rect(&mut self, rect: ObjectHandle) {
            self.rects.push(rect);
        }

        fn clear_selection(&mut self) {
            self.selected = None;
            self.dragging = false;
        }
        
        fn set_drag_offset(&mut self, x: f64, y: f64) {
            self.drag_offset_x = x;
            self.drag_offset_y = y;
        }
        
        fn start_dragging(&mut self, rect: ObjectHandle) {
            self.selected = Some(rect);
            self.dragging = true;
        }
        
        fn stop_dragging(&mut self) {
            self.dragging = false;
        }
        
        fn get_selected_handle(&mut self) -> i64 {
            match self.selected {
                Some(ObjectHandle(h)) => h as i64,
                None => -1,
            }
        }
        
        fn get_rects_count(&mut self) -> i64 {
            self.rects.len() as i64
        }
        
        fn get_rect_at(&mut self, index: i64) -> i64 {
            if index >= 0 && (index as usize) < self.rects.len() {
                let ObjectHandle(h) = self.rects[index as usize];
                h as i64
            } else {
                -1
            }
        }
        
        fn is_dragging(&mut self) -> bool {
            self.dragging
        }
    }
});