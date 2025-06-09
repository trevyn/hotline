hotline::object!({
    #[derive(Clone)]
    pub struct HighlightLens {
        #[setter]
        target: Option<Rect>,
        #[setter]
        #[default((0, 255, 0, 255))]
        highlight_color: (u8, u8, u8, u8), // BGRA
        #[setter]
        #[default(false)]
        show_handles: bool,
    }

    impl HighlightLens {
        fn draw_line(
            buffer: &mut [u8],
            bw: i64,
            bh: i64,
            pitch: i64,
            b: u8,
            g: u8,
            r: u8,
            a: u8,
            x0: f64,
            y0: f64,
            x1: f64,
            y1: f64,
        ) {
            let dx = x1 - x0;
            let dy = y1 - y0;
            let steps = dx.abs().max(dy.abs()).ceil() as i64;
            if steps == 0 {
                return;
            }
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                let x = x0 + dx * t;
                let y = y0 + dy * t;
                if x >= 0.0 && x < bw as f64 && y >= 0.0 && y < bh as f64 {
                    let offset = (y as i64 * pitch + x as i64 * 4) as usize;
                    if offset + 3 < buffer.len() {
                        buffer[offset] = b;
                        buffer[offset + 1] = g;
                        buffer[offset + 2] = r;
                        buffer[offset + 3] = a;
                    }
                }
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if let Some(ref mut target) = self.target {
                let (x, y, width, height) = target.bounds();

                let x_start = (x as i32).max(0) as u32;
                let y_start = (y as i32).max(0) as u32;
                let x_end = ((x + width) as i32).min(buffer_width as i32) as u32;
                let y_end = ((y + height) as i32).min(buffer_height as i32) as u32;

                let corners = target.corners();
                let (b, g, r, a) = self.highlight_color;

                for i in 0..4 {
                    let (x0, y0) = corners[i];
                    let (x1, y1) = corners[(i + 1) % 4];
                    Self::draw_line(buffer, buffer_width, buffer_height, pitch, b, g, r, a, x0, y0, x1, y1);
                }

                if self.show_handles {
                    let handle = 6u32;
                    let half = handle / 2;
                    let mid_x = (x_start + x_end) / 2;
                    let mid_y = (y_start + y_end) / 2;
                    let mut positions = vec![
                        (x_start, y_start),
                        (x_end.saturating_sub(handle), y_start),
                        (x_start, y_end.saturating_sub(handle)),
                        (x_end.saturating_sub(handle), y_end.saturating_sub(handle)),
                        (x_start, mid_y.saturating_sub(half)),
                        (x_end.saturating_sub(handle), mid_y.saturating_sub(half)),
                        (mid_x.saturating_sub(half), y_start),
                        (mid_x.saturating_sub(half), y_end.saturating_sub(handle)),
                    ];

                    for (hx, hy) in positions.drain(..) {
                        for py in hy..hy + handle {
                            for px in hx..hx + handle {
                                if px < buffer_width as u32 && py < buffer_height as u32 {
                                    let off = (py * (pitch as u32) + px * 4) as usize;
                                    if off + 3 < buffer.len() {
                                        buffer[off] = b;
                                        buffer[off + 1] = g;
                                        buffer[off + 2] = r;
                                        buffer[off + 3] = a;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
});
