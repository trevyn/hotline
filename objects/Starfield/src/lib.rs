hotline::object!({
    use rand::Rng;

    #[derive(Default)]
    pub struct Starfield {
        star_x: Vec<f32>,
        star_y: Vec<f32>,
        star_z: Vec<f32>,
        star_brightness: Vec<u8>,
        rect: Option<Rect>,
        controller_velocity: (f32, f32),
        base_velocity: (f32, f32),
        atlas_ids: Vec<Option<u32>>,
        initialized: bool,
    }

    impl Starfield {
        pub fn initialize(&mut self) {
            self.star_x.clear();
            self.star_y.clear();
            self.star_z.clear();
            self.star_brightness.clear();
            self.base_velocity = (0.0, 0.0);
            self.controller_velocity = (0.0, 0.0);
            self.initialized = true;
        }

        pub fn set_rect(&mut self, rect: Rect) {
            let (x, y, w, h) = rect.bounds();
            self.rect = Some(rect);

            // Initialize stars if not already done
            if self.star_x.is_empty() {
                let mut rng = rand::rng();
                let star_count = 300;

                for _ in 0..star_count {
                    self.star_x.push(rng.random_range(x..x + w) as f32);
                    self.star_y.push(rng.random_range(y..y + h) as f32);
                    self.star_z.push(rng.random_range(0.1..1.0));
                    self.star_brightness.push(rng.random_range(100..255));
                }
            }
        }

        pub fn update_controller(&mut self, left_x: f32, left_y: f32, _right_x: f32, _right_y: f32) {
            // Use left stick to control star movement
            self.controller_velocity = (left_x * 200.0, left_y * 200.0);
        }

        pub fn update(&mut self, _delta_time: f64) {
            if let Some(rect) = &self.rect {
                let (x, y, w, h) = rect.bounds();

                // Update star positions based on velocity
                let vx = self.base_velocity.0 + self.controller_velocity.0;
                let vy = self.base_velocity.1 + self.controller_velocity.1;

                for i in 0..self.star_x.len() {
                    // Apply velocity with parallax effect
                    self.star_x[i] -= vx * self.star_z[i] * 0.016;
                    self.star_y[i] -= vy * self.star_z[i] * 0.016;

                    // Wrap around screen edges
                    if self.star_x[i] < x as f32 {
                        self.star_x[i] += w as f32;
                    } else if self.star_x[i] > (x + w) as f32 {
                        self.star_x[i] -= w as f32;
                    }

                    if self.star_y[i] < y as f32 {
                        self.star_y[i] += h as f32;
                    } else if self.star_y[i] > (y + h) as f32 {
                        self.star_y[i] -= h as f32;
                    }
                }
            }
        }

        pub fn register_atlases(&mut self, gpu_renderer: &mut GPURenderer) {
            // Create different sized star textures
            let sizes = [1, 2, 3];

            for size in sizes.iter() {
                if self.atlas_ids.len() < sizes.len() {
                    let texture_size = *size;
                    let mut texture_data = vec![0u8; (texture_size * texture_size * 4) as usize];

                    // Create a simple star texture
                    for y in 0..texture_size {
                        for x in 0..texture_size {
                            let idx = ((y * texture_size + x) * 4) as usize;
                            // Simple white pixel for now
                            texture_data[idx] = 255; // R
                            texture_data[idx + 1] = 255; // G
                            texture_data[idx + 2] = 255; // B
                            texture_data[idx + 3] = 255; // A
                        }
                    }

                    let id = gpu_renderer.register_atlas(
                        texture_data,
                        texture_size as u32,
                        texture_size as u32,
                        AtlasFormat::RGBA,
                    );
                    self.atlas_ids.push(Some(id));
                }
            }
        }

        pub fn generate_commands(&mut self, gpu_renderer: &mut GPURenderer) {
            // Update stars
            self.update(0.016);

            // Make sure atlases are registered
            if self.atlas_ids.is_empty() {
                self.register_atlases(gpu_renderer);
            }

            if let Some(rect) = &self.rect {
                let (rx, ry, rw, rh) = rect.bounds();

                // Draw background
                let bg_atlas = self.atlas_ids.get(0).and_then(|id| *id);
                if let Some(atlas_id) = bg_atlas {
                    // Black background
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: atlas_id,
                        dest_x: rx,
                        dest_y: ry,
                        dest_width: rw,
                        dest_height: rh,
                        rotation: 0.0,
                        color: (255, 0, 0, 0), // ABGR: black
                    });
                }

                // Draw stars
                for i in 0..self.star_x.len() {
                    let z = self.star_z[i];
                    // Choose atlas based on depth (closer stars are bigger)
                    let atlas_idx = if z > 0.7 {
                        2
                    } else if z > 0.4 {
                        1
                    } else {
                        0
                    };

                    if let Some(Some(atlas_id)) = self.atlas_ids.get(atlas_idx) {
                        let size = (atlas_idx + 1) as f64;
                        let brightness = (self.star_brightness[i] as f32 * z) as u8;

                        gpu_renderer.add_command(RenderCommand::Rect {
                            texture_id: *atlas_id,
                            dest_x: self.star_x[i] as f64 - size / 2.0,
                            dest_y: self.star_y[i] as f64 - size / 2.0,
                            dest_width: size,
                            dest_height: size,
                            rotation: 0.0,
                            color: (255, brightness, brightness, brightness), // ABGR
                        });
                    }
                }
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            // GPU only
            let _ = (buffer, buffer_width, buffer_height, pitch);
        }
    }
});
