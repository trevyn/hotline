use hotline::HotlineObject;

hotline::object!({
    #[derive(Default)]
    pub struct ClickInspector {
        items: Vec<String>,
        renderers: Vec<TextRenderer>,
        #[default(10.0)]
        x: f64,
        #[default(10.0)]
        y: f64,
        dragging: bool,
        drag_offset_x: f64,
        drag_offset_y: f64,
        visible: bool,
    }

    impl ClickInspector {
        fn ensure_renderers(&mut self) {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }
            if self.renderers.len() != self.items.len() {
                self.renderers.clear();
                for item in &self.items {
                    self.renderers.push(TextRenderer::new().with_text(item.clone()).with_color((255, 255, 255, 255)));
                }
            } else {
                for (renderer, item) in self.renderers.iter_mut().zip(&self.items) {
                    renderer.set_text(item.clone());
                }
            }
        }

        pub fn open(&mut self, items: Vec<String>) {
            self.items = items;
            self.visible = true;
            self.ensure_renderers();
        }

        pub fn close(&mut self) {
            self.visible = false;
        }

        pub fn is_dragging(&self) -> bool {
            self.dragging
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) -> bool {
            if !self.visible {
                return false;
            }
            let item_height = 16.0;
            let width = 200.0;
            let height = item_height * self.items.len() as f64;
            if x >= self.x && x <= self.x + width && y >= self.y && y <= self.y + height {
                self.dragging = true;
                self.drag_offset_x = x - self.x;
                self.drag_offset_y = y - self.y;
                return true;
            }
            false
        }

        pub fn handle_mouse_up(&mut self) {
            self.dragging = false;
        }

        pub fn handle_mouse_move(&mut self, x: f64, y: f64) {
            if self.dragging {
                self.x = x - self.drag_offset_x;
                self.y = y - self.drag_offset_y;
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if !self.visible {
                return;
            }
            self.ensure_renderers();
            let item_height = 16.0;
            for (i, renderer) in self.renderers.iter_mut().enumerate() {
                renderer.set_x(self.x);
                renderer.set_y(self.y + i as f64 * item_height);
                renderer.render(buffer, buffer_width, buffer_height, pitch);
            }
        }
    }
});
