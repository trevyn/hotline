hotline::object!({
    #[derive(Clone)]
    pub struct HighlightLens {
        #[setter]
        target: Option<Rect>,
        #[setter]
        #[default((0, 255, 0, 255))]
        highlight_color: (u8, u8, u8, u8), // BGRA
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
            let steps = dx.abs().max(dy.abs()) as i64;
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
                let corners = target.corners();
                let (b, g, r, a) = self.highlight_color;

                for i in 0..4 {
                    let (x0, y0) = corners[i];
                    let (x1, y1) = corners[(i + 1) % 4];
                    Self::draw_line(buffer, buffer_width, buffer_height, pitch, b, g, r, a, x0, y0, x1, y1);
                }
            }
        }
    }
});
