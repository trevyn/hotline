use hotline::HotlineObject;

hotline::object!({
    #[derive(Clone, Default)]
    pub struct Checkbox {
        rect: Option<Rect>,
        label: Option<TextRenderer>,
        #[setter]
        #[default(false)]
        checked: bool,
    }

    impl Checkbox {
        pub fn set_rect(&mut self, rect: Rect) {
            self.rect = Some(rect);
        }

        pub fn set_label(&mut self, text: String) {
            if self.label.is_none() {
                if let Some(registry) = self.get_registry() {
                    ::hotline::set_library_registry(registry);
                }
                self.label = Some(TextRenderer::new());
            }
            if let Some(ref mut tr) = self.label {
                tr.set_text(text);
                if let Some(ref mut r) = self.rect {
                    let (x, y, w, _) = r.bounds();
                    tr.set_x(x + w + 5.0);
                    tr.set_y(y);
                }
            }
        }

        pub fn checked(&mut self) -> bool {
            self.checked
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) {
            if let Some(ref mut r) = self.rect {
                if r.contains_point(x, y) {
                    self.checked = !self.checked;
                }
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], bw: i64, bh: i64, pitch: i64) {
            if let Some(ref mut r) = self.rect {
                r.render(buffer, bw, bh, pitch);
                if self.checked {
                    let (x, y, w, h) = r.bounds();
                    let x = x as i64;
                    let y = y as i64;
                    let w = w as i64;
                    let h = h as i64;
                    let min_dim = w.min(h);
                    for i in 0..min_dim {
                        let px1 = x + i;
                        let py1 = y + i;
                        let px2 = x + i;
                        let py2 = y + h - 1 - i;
                        if px1 >= 0 && px1 < bw && py1 >= 0 && py1 < bh {
                            let off = (py1 * pitch + px1 * 4) as usize;
                            if off + 3 < buffer.len() {
                                buffer[off] = 255;
                                buffer[off + 1] = 255;
                                buffer[off + 2] = 255;
                                buffer[off + 3] = 255;
                            }
                        }
                        if px2 >= 0 && px2 < bw && py2 >= 0 && py2 < bh {
                            let off = (py2 * pitch + px2 * 4) as usize;
                            if off + 3 < buffer.len() {
                                buffer[off] = 255;
                                buffer[off + 1] = 255;
                                buffer[off + 2] = 255;
                                buffer[off + 3] = 255;
                            }
                        }
                    }
                }
            }
            if let Some(ref mut tr) = self.label {
                tr.render(buffer, bw, bh, pitch);
            }
        }
    }
});
