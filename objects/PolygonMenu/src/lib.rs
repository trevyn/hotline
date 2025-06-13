hotline::object!({
    #[derive(Default)]
    pub struct PolygonMenu {
        renderers: Vec<TextRenderer>,
        x: f64,
        y: f64,
        visible: bool,
        hover: Option<usize>,
    }

    impl PolygonMenu {
        pub fn open(&mut self, x: f64, y: f64) {
            if self.renderers.is_empty() {
                if let Some(registry) = self.get_registry() {
                    ::hotline::set_library_registry(registry);
                }
                for i in 3..=10 {
                    self.renderers.push(TextRenderer::new().with_text(i.to_string()).with_color((255, 255, 255, 255)));
                }
            }
            self.x = x;
            self.y = y;
            self.visible = true;
            self.hover = None;
        }

        pub fn close(&mut self) {
            self.visible = false;
            self.hover = None;
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) -> Option<i64> {
            if !self.visible {
                return None;
            }
            let item_height = 16.0;
            for (i, _) in self.renderers.iter().enumerate() {
                let iy = self.y + i as f64 * item_height;
                if x >= self.x && x <= self.x + 100.0 && y >= iy && y <= iy + item_height {
                    self.visible = false;
                    return Some(i as i64 + 3);
                }
            }
            self.visible = false;
            None
        }

        pub fn handle_mouse_move(&mut self, x: f64, y: f64) {
            if !self.visible {
                return;
            }
            let item_height = 16.0;
            self.hover = None;
            for (i, _) in self.renderers.iter().enumerate() {
                let iy = self.y + i as f64 * item_height;
                if x >= self.x && x <= self.x + 100.0 && y >= iy && y <= iy + item_height {
                    self.hover = Some(i);
                    break;
                }
            }
        }

        fn preview_sides(&self) -> Option<i64> {
            self.hover.map(|i| i as i64 + 3)
        }

        pub fn render(&mut self, buffer: &mut [u8], bw: i64, bh: i64, pitch: i64) {
            if !self.visible {
                return;
            }
            let item_height = 16.0;
            for (i, r) in self.renderers.iter_mut().enumerate() {
                r.set_x(self.x);
                r.set_y(self.y + i as f64 * item_height);
                r.render(buffer, bw, bh, pitch);
            }
            if let Some(sides) = self.preview_sides() {
                let mut preview = RegularPolygon::new();
                if let Some(idx) = self.hover {
                    let py = self.y + idx as f64 * item_height + item_height / 2.0;
                    preview.initialize(self.x + 120.0, py, 20.0, sides);
                    preview.render(buffer, bw, bh, pitch);
                }
            }
        }

        pub fn is_visible(&self) -> bool {
            self.visible
        }
    }
});
