hotline::object!({
    #[derive(Default, Clone)]
    pub struct Rect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        #[default(0.0)]
        rotation: f64, // radians
    }

    impl Rect {
        pub fn initialize(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.x = x;
            self.y = y;
            self.width = width;
            self.height = height;
            self.rotation = 0.0;
        }

        pub fn contains_point(&mut self, point_x: f64, point_y: f64) -> bool {
            let (cx, cy) = self.center();
            let (sin_r, cos_r) = self.rotation.sin_cos();
            let dx = point_x - cx;
            let dy = point_y - cy;
            let rx = dx * cos_r + dy * sin_r;
            let ry = -dx * sin_r + dy * cos_r;
            rx.abs() <= self.width / 2.0 && ry.abs() <= self.height / 2.0
        }

        pub fn position(&mut self) -> (f64, f64) {
            (self.x, self.y)
        }

        pub fn bounds(&mut self) -> (f64, f64, f64, f64) {
            let corners = self.corners();
            let xs = [corners[0].0, corners[1].0, corners[2].0, corners[3].0];
            let ys = [corners[0].1, corners[1].1, corners[2].1, corners[3].1];
            let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
            let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            (min_x, min_y, max_x - min_x, max_y - min_y)
        }

        pub fn move_by(&mut self, dx: f64, dy: f64) {
            self.x += dx;
            self.y += dy;
        }

        pub fn set_rotation(&mut self, angle: f64) {
            self.rotation = angle;
        }

        pub fn rotation(&mut self) -> f64 {
            self.rotation
        }

        pub fn center(&mut self) -> (f64, f64) {
            (self.x + self.width / 2.0, self.y + self.height / 2.0)
        }

        pub fn corners(&mut self) -> [(f64, f64); 4] {
            let (cx, cy) = self.center();
            let hw = self.width / 2.0;
            let hh = self.height / 2.0;
            let (sin_r, cos_r) = self.rotation.sin_cos();
            let mut rot = |dx: f64, dy: f64| -> (f64, f64) {
                let rx = dx * cos_r - dy * sin_r;
                let ry = dx * sin_r + dy * cos_r;
                (cx + rx, cy + ry)
            };
            [rot(-hw, -hh), rot(hw, -hh), rot(hw, hh), rot(-hw, hh)]
        }

        pub fn resize(&mut self, x: f64, y: f64, width: f64, height: f64) {
            self.x = x;
            self.y = y;
            self.width = width;
            self.height = height;
        }

        pub fn info_lines(&mut self) -> Vec<String> {
            vec![
                "Rect".to_string(),
                format!("  x: {:.1}", self.x),
                format!("  y: {:.1}", self.y),
                format!("  width: {:.1}", self.width),
                format!("  height: {:.1}", self.height),
                format!("  rotation: {:.2}", self.rotation),
            ]
        }

        pub fn render_offset(
            &mut self,
            buffer: &mut [u8],
            buffer_width: i64,
            buffer_height: i64,
            pitch: i64,
            offset_x: f64,
            offset_y: f64,
        ) {
            let (bx, by, bw, bh) = self.bounds();
            let x_start = ((bx - offset_x) as i32).max(0) as u32;
            let y_start = ((by - offset_y) as i32).max(0) as u32;
            let x_end = ((bx - offset_x + bw) as i32).min(buffer_width as i32) as u32;
            let y_end = ((by - offset_y + bh) as i32).min(buffer_height as i32) as u32;

            let (cx, cy) = self.center();
            let cx = cx - offset_x;
            let cy = cy - offset_y;
            let (sin_r, cos_r) = self.rotation.sin_cos();

            for y in y_start..y_end {
                for x in x_start..x_end {
                    let dx = x as f64 - cx;
                    let dy = y as f64 - cy;
                    let rx = dx * cos_r + dy * sin_r;
                    let ry = -dx * sin_r + dy * cos_r;
                    if rx.abs() <= self.width / 2.0 && ry.abs() <= self.height / 2.0 {
                        let offset = (y * (pitch as u32) + x * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = 120; // B
                            buffer[offset + 1] = 0; // G
                            buffer[offset + 2] = 0; // R
                            buffer[offset + 3] = 255; // A
                        }
                    }
                }
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            self.render_offset(buffer, buffer_width, buffer_height, pitch, 0.0, 0.0);
        }
    }
});
