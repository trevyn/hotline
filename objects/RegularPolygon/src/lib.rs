hotline::object!({
    #[derive(Default, Clone)]
    pub struct RegularPolygon {
        #[setter]
        #[default(0.0)]
        x: f64,
        #[setter]
        #[default(0.0)]
        y: f64,
        #[setter]
        #[default(10.0)]
        radius: f64,
        #[setter]
        #[default(3)]
        sides: i64,
        #[setter]
        #[default(0.0)]
        rotation: f64,
        #[setter]
        #[default((255,0,0,255))]
        color: (u8, u8, u8, u8),
    }

    impl RegularPolygon {
        pub fn initialize(&mut self, x: f64, y: f64, radius: f64, sides: i64) {
            self.x = x;
            self.y = y;
            self.radius = radius;
            self.sides = sides.max(3);
        }

        fn vertices(&self) -> Vec<(f64, f64)> {
            let mut verts = Vec::new();
            let sides = self.sides.max(3) as usize;
            for i in 0..sides {
                let angle = 2.0 * std::f64::consts::PI * (i as f64) / (sides as f64) + self.rotation;
                let vx = self.x + self.radius * angle.cos();
                let vy = self.y + self.radius * angle.sin();
                verts.push((vx, vy));
            }
            verts
        }

        pub fn contains_point(&self, x: f64, y: f64) -> bool {
            let verts = self.vertices();
            self.point_in_polygon(x, y, &verts)
        }

        fn point_in_polygon(&self, px: f64, py: f64, verts: &[(f64, f64)]) -> bool {
            let mut inside = false;
            let mut j = verts.len() - 1;
            for i in 0..verts.len() {
                let (xi, yi) = verts[i];
                let (xj, yj) = verts[j];
                let intersect = ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi);
                if intersect {
                    inside = !inside;
                }
                j = i;
            }
            inside
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if self.radius <= 0.0 {
                return;
            }
            let verts = self.vertices();
            let min_x = verts.iter().map(|(x, _)| *x).fold(std::f64::INFINITY, f64::min).floor().max(0.0) as i32;
            let max_x =
                verts.iter().map(|(x, _)| *x).fold(std::f64::NEG_INFINITY, f64::max).ceil().min(buffer_width as f64)
                    as i32;
            let min_y = verts.iter().map(|(_, y)| *y).fold(std::f64::INFINITY, f64::min).floor().max(0.0) as i32;
            let max_y =
                verts.iter().map(|(_, y)| *y).fold(std::f64::NEG_INFINITY, f64::max).ceil().min(buffer_height as f64)
                    as i32;

            let (b, g, r, a) = self.color;

            for y in min_y..max_y {
                for x in min_x..max_x {
                    if self.point_in_polygon(x as f64 + 0.5, y as f64 + 0.5, &verts) {
                        let offset = (y as u32 * pitch as u32 + x as u32 * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = b;
                            buffer[offset + 1] = g;
                            buffer[offset + 2] = r;
                            buffer[offset + 3] = a;
                        }
                    }
                }
            }
        }
    }
});
