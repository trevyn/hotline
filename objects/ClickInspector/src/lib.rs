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
