hotline::object!({
    #[derive(Clone, Default)]
    pub struct Image {
        #[default(0.0)]
        x: f64,
        #[default(0.0)]
        y: f64,
        width: u32,
        height: u32,
        data: Vec<u8>,
    }

    impl Image {
        pub fn initialize(&mut self, x: f64, y: f64) {
            self.x = x;
            self.y = y;
        }

        fn load_from_loader(&mut self, loader: &mut PNGLoader) -> Result<(), String> {
            if let Some((data, width, height)) = loader.data() {
                self.width = width;
                self.height = height;
                // Convert from RGBA to BGRA
                self.data = data.chunks_exact(4).flat_map(|px| [px[2], px[1], px[0], px[3]]).collect();
                Ok(())
            } else {
                Err("PNG data not available".to_string())
            }
        }

        pub fn load_png(&mut self, path: &str) -> Result<(), String> {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }
            let mut loader = PNGLoader::new();
            loader.load_png(path)?;
            self.load_from_loader(&mut loader)
        }

        pub fn load_png_bytes(&mut self, data: &[u8]) -> Result<(), String> {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }
            let mut loader = PNGLoader::new();
            loader.load_png_bytes(data)?;
            self.load_from_loader(&mut loader)
        }

        pub fn render(&mut self, buffer: &mut [u8], bw: i64, bh: i64, pitch: i64) {
            let x_start = self.x.max(0.0) as i64;
            let y_start = self.y.max(0.0) as i64;
            let x_end = (self.x + self.width as f64).min(bw as f64) as i64;
            let y_end = (self.y + self.height as f64).min(bh as f64) as i64;

            for y in y_start..y_end {
                for x in x_start..x_end {
                    let src_x = (x - self.x as i64) as usize;
                    let src_y = (y - self.y as i64) as usize;
                    let src_off = (src_y * self.width as usize + src_x) * 4;
                    let dst_off = (y * pitch as i64 + x * 4) as usize;
                    if src_off + 3 < self.data.len() && dst_off + 3 < buffer.len() {
                        buffer[dst_off..dst_off + 4].copy_from_slice(&self.data[src_off..src_off + 4]);
                    }
                }
            }
        }

        pub fn bounds(&mut self) -> (f64, f64, f64, f64) {
            (self.x, self.y, self.width as f64, self.height as f64)
        }
    }
});
