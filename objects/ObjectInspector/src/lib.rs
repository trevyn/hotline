use hotline::HotlineObject;

hotline::object!({
    #[derive(Default)]
    pub struct ObjectInspector {
        rect: Option<Rect>,
        renderers: Vec<TextRenderer>,
        title_renderer: Option<TextRenderer>,
        fields: Vec<(String, String)>,
        title: String,
    }

    impl ObjectInspector {
        pub fn set_rect(&mut self, rect: Rect) {
            self.rect = Some(rect);
        }

        pub fn inspect(&mut self, name: &str, fields: Vec<(String, String)>) {
            self.title = name.to_string();
            self.fields = fields;
            self.ensure_renderers();
        }

        fn ensure_renderers(&mut self) {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }
            if self.title_renderer.is_none() {
                self.title_renderer = Some(TextRenderer::new());
            }
            while self.renderers.len() < self.fields.len() {
                self.renderers.push(TextRenderer::new());
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], bw: i64, bh: i64, pitch: i64) {
            if let Some(ref mut rect) = self.rect {
                let (x, y, _, _) = rect.bounds();
                if let Some(ref mut title) = self.title_renderer {
                    title.set_text(self.title.clone());
                    title.set_x(x + 5.0);
                    title.set_y(y + 5.0);
                    title.render(buffer, bw, bh, pitch);
                }

                for (i, (name, value)) in self.fields.iter().enumerate() {
                    if let Some(tr) = self.renderers.get_mut(i) {
                        tr.set_text(format!("{}: {}", name, value));
                        tr.set_x(x + 5.0);
                        tr.set_y(y + 25.0 + i as f64 * 14.0);
                        tr.render(buffer, bw, bh, pitch);
                    }
                }
            }
        }
    }
});
