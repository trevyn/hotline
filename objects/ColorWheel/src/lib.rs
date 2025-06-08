hotline::object!({
    #[derive(Clone, Default)]
    pub struct ColorWheel {
        rect: Option<Rect>,
        #[setter]
        #[default((255, 255, 255, 255))]
        selected_color: (u8, u8, u8, u8),
        #[default(false)]
        dragging: bool,
    }

    impl ColorWheel {
        pub fn set_rect(&mut self, rect: Rect) {
            self.rect = Some(rect);
        }

        pub fn selected_color(&mut self) -> (u8, u8, u8, u8) {
            self.selected_color
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) -> Option<(u8, u8, u8, u8)> {
            if let Some(color) = self.update_color_at(x, y) {
                self.dragging = true;
                return Some(color);
            }
            None
        }

        pub fn handle_mouse_up(&mut self) {
            self.dragging = false;
        }

        pub fn handle_mouse_move(&mut self, x: f64, y: f64) -> Option<(u8, u8, u8, u8)> {
            if self.dragging {
                return self.update_color_at(x, y);
            }
            None
        }

        fn update_color_at(&mut self, x: f64, y: f64) -> Option<(u8, u8, u8, u8)> {
            if let Some(ref mut r) = self.rect {
                if r.contains_point(x, y) {
                    let (rx, ry, w, h) = r.bounds();
                    let cx = rx + w / 2.0;
                    let cy = ry + h / 2.0;
                    let radius = w.min(h) / 2.0;
                    let dx = x - cx;
                    let dy = y - cy;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist <= radius {
                        let h = (dy.atan2(dx) + std::f64::consts::PI) / (2.0 * std::f64::consts::PI);
                        let s = dist / radius;
                        let v = 1.0;
                        let (r_col, g_col, b_col) = Self::hsv_to_rgb(h, s, v);
                        self.selected_color = (b_col, g_col, r_col, 255);
                        return Some(self.selected_color);
                    }
                }
            }
            None
        }

        fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
            let i = (h * 6.0).floor();
            let f = h * 6.0 - i;
            let p = v * (1.0 - s);
            let q = v * (1.0 - f * s);
            let t = v * (1.0 - (1.0 - f) * s);
            let (r, g, b) = match i as i32 % 6 {
                0 => (v, t, p),
                1 => (q, v, p),
                2 => (p, v, t),
                3 => (p, q, v),
                4 => (t, p, v),
                _ => (v, p, q),
            };
            ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if let Some(ref mut rect) = self.rect {
                let (x, y, w, h) = rect.bounds();
                let cx = x + w / 2.0;
                let cy = y + h / 2.0;
                let radius = w.min(h) / 2.0;

                let x_start = x.max(0.0) as u32;
                let y_start = y.max(0.0) as u32;
                let x_end = (x + w).min(buffer_width as f64) as u32;
                let y_end = (y + h).min(buffer_height as f64) as u32;

                for py in y_start..y_end {
                    for px in x_start..x_end {
                        let dx = px as f64 + 0.5 - cx;
                        let dy = py as f64 + 0.5 - cy;
                        let dist = (dx * dx + dy * dy).sqrt();
                        if dist <= radius {
                            let h_val = (dy.atan2(dx) + std::f64::consts::PI) / (2.0 * std::f64::consts::PI);
                            let s_val = dist / radius;
                            let v_val = 1.0;
                            let (r_col, g_col, b_col) = Self::hsv_to_rgb(h_val, s_val, v_val);
                            let offset = (py * (pitch as u32) + px * 4) as usize;
                            if offset + 3 < buffer.len() {
                                buffer[offset] = b_col;
                                buffer[offset + 1] = g_col;
                                buffer[offset + 2] = r_col;
                                buffer[offset + 3] = 255;
                            }
                        }
                    }
                }
            }
        }
    }
});
