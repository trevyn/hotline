use hotline::HotlineObject;

hotline::object!({
    #[derive(Default)]
    pub struct ContextMenu {
        items: Vec<String>,
        renderers: Vec<TextRenderer>,
        x: f64,
        y: f64,
        visible: bool,
    }

    impl ContextMenu {
        fn initialize(&mut self) {
            if self.items.is_empty() {
                self.items = vec!["Rect".to_string(), "RegularPolygon".to_string()];
            }

            if self.renderers.is_empty() {
                if let Some(registry) = self.get_registry() {
                    ::hotline::set_library_registry(registry);
                }
                for item in &self.items {
                    self.renderers.push(TextRenderer::new().with_text(item.clone()).with_color((255, 255, 255, 255)));
                }
            }
        }

        pub fn open(&mut self, x: f64, y: f64) {
            self.initialize();
            self.x = x;
            self.y = y;
            self.visible = true;
        }

        pub fn open_with_items(&mut self, items: Vec<String>, x: f64, y: f64) {
            self.set_items(items);
            self.x = x;
            self.y = y;
            self.visible = true;
        }

        pub fn close(&mut self) {
            self.visible = false;
        }

        pub fn set_items(&mut self, items: Vec<String>) {
            self.items = items;
            self.renderers.clear();
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }
            for item in &self.items {
                self.renderers.push(TextRenderer::new().with_text(item.clone()).with_color((255, 255, 255, 255)));
            }
        }

        pub fn is_visible(&self) -> bool {
            self.visible
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) -> Option<String> {
            if !self.visible {
                return None;
            }
            let item_height = 16.0;
            let mut cursor_y = self.y;
            let mut result = None;
            for item in &self.items {
                if x >= self.x && x <= self.x + 100.0 && y >= cursor_y && y <= cursor_y + item_height {
                    result = Some(item.clone());
                    break;
                }
                cursor_y += item_height;
            }
            self.visible = false;
            result
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if !self.visible {
                return;
            }
            self.initialize();
            let item_height = 16.0;
            for (i, renderer) in self.renderers.iter_mut().enumerate() {
                renderer.set_x(self.x);
                renderer.set_y(self.y + i as f64 * item_height);
                renderer.render(buffer, buffer_width, buffer_height, pitch);
            }
        }
    }
});
