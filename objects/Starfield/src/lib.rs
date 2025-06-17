hotline::object!({
    use rand::Rng;

    #[derive(Default, Clone)]
    pub struct Starfield {
        star_x: Vec<f32>,
        star_y: Vec<f32>,
        star_z: Vec<f32>,
        star_brightness: Vec<u8>,
        rect: Option<Rect>,
        controller_velocity: (f32, f32),
        base_velocity: (f32, f32),
        z_velocity: f32, // Forward/backward velocity
        acceleration_multiplier: f32,
        atlas_ids: Vec<Option<u32>>,
        border_atlas_id: Option<u32>,
        initialized: bool,
        dragging: bool,
        resize_mode: Option<u8>, // 0=None, 1=Top, 2=Bottom, 3=Left, 4=Right, 5=TopLeft, 6=TopRight, 7=BottomLeft, 8=BottomRight
        drag_offset: (f64, f64),
        speed_display: Option<TextRenderer>,
    }

    impl Starfield {
        pub fn initialize(&mut self) {
            self.star_x.clear();
            self.star_y.clear();
            self.star_z.clear();
            self.star_brightness.clear();
            self.base_velocity = (0.0, 0.0);
            self.controller_velocity = (0.0, 0.0);
            self.z_velocity = 0.0;
            self.acceleration_multiplier = 5.0;
            self.initialized = true;
            self.dragging = false;
            self.resize_mode = None;
            self.drag_offset = (0.0, 0.0);

            // Initialize speed display
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }
            let mut display = TextRenderer::new();
            display.set_text(format!("Speed: {:.1}x", self.acceleration_multiplier));
            display.set_color((255, 255, 255, 255));
            self.speed_display = Some(display);
        }

        pub fn set_rect(&mut self, rect: Rect) {
            let (x, y, w, h) = rect.bounds();
            self.rect = Some(rect);

            // Initialize stars if not already done
            if self.star_x.is_empty() {
                let mut rng = rand::rng();
                let star_count = 300;
                let center_x = x + w / 2.0;
                let center_y = y + h / 2.0;

                for _ in 0..star_count {
                    // Distribute stars with slight bias toward center for better initial look
                    let angle = rng.random_range(0.0..std::f32::consts::TAU);
                    let u: f32 = rng.random_range(0.0..1.0);
                    let max_radius = (w.min(h) / 2.0) as f32;
                    // Use sqrt for more uniform initial distribution
                    let radius = max_radius * u.sqrt();

                    self.star_x.push(center_x as f32 + angle.cos() * radius);
                    self.star_y.push(center_y as f32 + angle.sin() * radius);
                    self.star_z.push(rng.random_range(0.01..1.0));
                    self.star_brightness.push(rng.random_range(100..255));
                }
            }
        }

        pub fn update_controller(
            &mut self,
            left_x: f32,
            left_y: f32,
            _right_x: f32,
            _right_y: f32,
            _left_trigger: f32,
            right_trigger: f32,
        ) {
            // Use left stick to control star movement
            self.controller_velocity = (left_x * 200.0, left_y * 200.0);
            // Use right trigger to accelerate forward into the starfield
            self.z_velocity = right_trigger * self.acceleration_multiplier;
        }

        pub fn set_acceleration_multiplier(&mut self, multiplier: f32) {
            self.acceleration_multiplier = multiplier;
            if let Some(ref mut display) = self.speed_display {
                display.set_text(format!("Speed: {:.1}x", multiplier));
            }
        }

        pub fn acceleration_multiplier(&self) -> f32 {
            self.acceleration_multiplier
        }

        pub fn update(&mut self, _delta_time: f64) {
            if let Some(rect) = &self.rect {
                let (x, y, w, h) = rect.bounds();
                let center_x = x + w / 2.0;
                let center_y = y + h / 2.0;

                // Update star positions based on velocity
                let vx = self.base_velocity.0 + self.controller_velocity.0;
                let vy = self.base_velocity.1 + self.controller_velocity.1;

                // Debug z_velocity
                static mut LAST_Z_VEL: f32 = 0.0;
                unsafe {
                    if (self.z_velocity - LAST_Z_VEL).abs() > 0.1 {
                        eprintln!("z_velocity changed: {:.2} -> {:.2}", LAST_Z_VEL, self.z_velocity);
                        LAST_Z_VEL = self.z_velocity;
                    }
                }

                for i in 0..self.star_x.len() {
                    // Apply lateral velocity with parallax effect
                    self.star_x[i] -= vx * self.star_z[i] * 0.016;
                    self.star_y[i] -= vy * self.star_z[i] * 0.016;

                    // Apply z-velocity (acceleration into starfield)
                    if self.z_velocity > 0.0 {
                        // Stars move towards us, getting bigger and brighter
                        // Moderate z movement for visible warp effect
                        self.star_z[i] += self.z_velocity * 0.016 * 1.0;

                        if self.star_z[i] > 1.0 {
                            // Reset star to far distance
                            self.star_z[i] = 0.01;
                            let mut rng = rand::rng();

                            // During warp, spawn stars across entire field but weighted toward center
                            if self.z_velocity > 0.5 {
                                // Use a gaussian-like distribution - more stars near center, but some everywhere
                                let angle = rng.random_range(0.0..std::f32::consts::TAU);
                                // Exponential distribution for radius - more likely to be near center
                                let u: f32 = rng.random_range(0.0..1.0);
                                let max_radius = (w.min(h) / 2.0) as f32;
                                let radius = -max_radius * (1.0_f32 - u).ln();
                                let radius = radius.min(max_radius * 0.9); // Cap at 90% of max to stay in bounds

                                self.star_x[i] = center_x as f32 + angle.cos() * radius;
                                self.star_y[i] = center_y as f32 + angle.sin() * radius;
                            } else {
                                // Normal distribution across field
                                self.star_x[i] = rng.random_range(x..x + w) as f32;
                                self.star_y[i] = rng.random_range(y..y + h) as f32;
                            }
                        }

                        // Create streaking effect by moving stars outward from center
                        let dx = self.star_x[i] - center_x as f32;
                        let dy = self.star_y[i] - center_y as f32;
                        let dist = (dx * dx + dy * dy).sqrt();

                        if dist > 0.01 {
                            // Normalize direction
                            let ndx = dx / dist;
                            let ndy = dy / dist;

                            // Speed based on z (closer = faster) and velocity
                            // Much slower acceleration to keep stars visible
                            let speed = self.z_velocity * self.star_z[i] * 20.0 * 0.016;
                            self.star_x[i] += ndx * speed;
                            self.star_y[i] += ndy * speed;

                            // Debug first star movement
                            // if i == 0 {
                            //     eprintln!("Update star 0: z={:.2} speed={:.1} move=({:.1},{:.1})",
                            //         self.star_z[i], speed, ndx * speed, ndy * speed);
                            // }
                        }
                    }

                    // Wrap around screen edges
                    if self.star_x[i] < x as f32 - 50.0
                        || self.star_x[i] > (x + w) as f32 + 50.0
                        || self.star_y[i] < y as f32 - 50.0
                        || self.star_y[i] > (y + h) as f32 + 50.0
                    {
                        // Reset position
                        let mut rng = rand::rng();
                        self.star_z[i] = 0.01;

                        if self.z_velocity > 0.5 {
                            // During warp, use same distribution as z-reset
                            let angle = rng.random_range(0.0..std::f32::consts::TAU);
                            let u: f32 = rng.random_range(0.0..1.0);
                            let max_radius = (w.min(h) / 2.0) as f32;
                            let radius = -max_radius * (1.0_f32 - u).ln();
                            let radius = radius.min(max_radius * 0.9);

                            self.star_x[i] = center_x as f32 + angle.cos() * radius;
                            self.star_y[i] = center_y as f32 + angle.sin() * radius;
                        } else {
                            // Normal movement - respawn anywhere in field
                            self.star_x[i] = rng.random_range(x..x + w) as f32;
                            self.star_y[i] = rng.random_range(y..y + h) as f32;
                        }
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

            // Create border atlas if not already created
            if self.border_atlas_id.is_none() {
                let border_pixel = vec![100u8, 100, 255, 255]; // Light blue border
                let id = gpu_renderer.register_atlas(border_pixel, 1, 1, AtlasFormat::RGBA);
                self.border_atlas_id = Some(id);
            }

            // Register speed display atlas
            if let Some(ref mut display) = self.speed_display {
                display.register_atlas(gpu_renderer);
            }
        }

        pub fn generate_commands(&mut self, gpu_renderer: &mut GPURenderer) {
            // Update stars
            self.update(0.016);

            // Make sure atlases are registered
            if self.atlas_ids.is_empty() {
                self.register_atlases(gpu_renderer);
            }

            // Debug when accelerating
            static mut LAST_MODE: bool = false;
            unsafe {
                let accelerating = self.z_velocity > 0.1;
                if accelerating != LAST_MODE {
                    eprintln!("Starfield mode changed: accelerating={} z_vel={:.2}", accelerating, self.z_velocity);
                    LAST_MODE = accelerating;
                }
            }

            if let Some(rect) = &self.rect {
                let (rx, ry, rw, rh) = rect.bounds();

                // Debug rect bounds once
                static mut PRINTED_BOUNDS: bool = false;
                unsafe {
                    if !PRINTED_BOUNDS {
                        eprintln!("Starfield bounds: ({:.0},{:.0}) {}x{}", rx, ry, rw, rh);
                        PRINTED_BOUNDS = true;
                    }
                }

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

                // Draw border
                if let Some(border_id) = self.border_atlas_id {
                    let border_width = 2.0;

                    // Top border
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: rx,
                        dest_y: ry,
                        dest_width: rw,
                        dest_height: border_width,
                        rotation: 0.0,
                        color: (255, 255, 255, 255),
                    });

                    // Bottom border
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: rx,
                        dest_y: ry + rh - border_width,
                        dest_width: rw,
                        dest_height: border_width,
                        rotation: 0.0,
                        color: (255, 255, 255, 255),
                    });

                    // Left border
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: rx,
                        dest_y: ry,
                        dest_width: border_width,
                        dest_height: rh,
                        rotation: 0.0,
                        color: (255, 255, 255, 255),
                    });

                    // Right border
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: rx + rw - border_width,
                        dest_y: ry,
                        dest_width: border_width,
                        dest_height: rh,
                        rotation: 0.0,
                        color: (255, 255, 255, 255),
                    });
                }

                // Draw stars
                let mut visible_count = 0;
                let mut streak_count = 0;

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
                        // Debug atlas availability
                        // if i == 0 && self.z_velocity > 0.1 {
                        //     eprintln!("Star 0: atlas_idx={} atlas_id={} z={:.2}", atlas_idx, atlas_id, z);
                        // }
                        let base_size = (atlas_idx + 1) as f64;
                        let size = base_size * (1.0 + z as f64 * 2.0); // Bigger when closer
                        let brightness = (self.star_brightness[i] as f32 * z).min(255.0) as u8;

                        let star_x = self.star_x[i] as f64;
                        let star_y = self.star_y[i] as f64;

                        // Check if star is within visible bounds
                        if star_x >= rx && star_x <= rx + rw && star_y >= ry && star_y <= ry + rh {
                            visible_count += 1;
                        }

                        // When accelerating, draw as streaks. Otherwise draw as dots.
                        if self.z_velocity > 0.1 {
                            // Calculate direction from center
                            let center_x = rx + rw / 2.0;
                            let center_y = ry + rh / 2.0;
                            let dx = star_x - center_x;
                            let dy = star_y - center_y;
                            let dist = (dx * dx + dy * dy).sqrt();

                            // Always draw stars when accelerating
                            streak_count += 1;

                            // For very small distances from center, just draw a dot
                            if dist < 5.0 {
                                gpu_renderer.add_command(RenderCommand::Rect {
                                    texture_id: *atlas_id,
                                    dest_x: star_x - size / 2.0,
                                    dest_y: star_y - size / 2.0,
                                    dest_width: size,
                                    dest_height: size,
                                    rotation: 0.0,
                                    color: (255, brightness, brightness, brightness),
                                });
                            } else {
                                // Streak length based on velocity, z depth, and a base length
                                let base_streak = 10.0;
                                let streak_length = base_streak + (self.z_velocity * z * 100.0) as f64;

                                // Normalize direction
                                let ndx = dx / dist;
                                let ndy = dy / dist;

                                // Line goes from behind the star to current position
                                // The streak trails behind the star's motion
                                let x1 = star_x - ndx * streak_length;
                                let y1 = star_y - ndy * streak_length;
                                let x2 = star_x;
                                let y2 = star_y;

                                // Debug info for first few stars only
                                // if i < 2 {
                                //     let r = ((star_x - center_x).powi(2) + (star_y - center_y).powi(2)).sqrt();
                                //     eprintln!("Star {}: pos({:.0},{:.0}) r={:.0} z={:.2} vel={:.1} streak={:.0}",
                                //         i, star_x, star_y, r, z, self.z_velocity, streak_length);
                                // }

                                // Brighter at the front of the streak
                                let front_brightness = ((brightness as f32) * 1.5).min(255.0) as u8;

                                // Draw the streak
                                let thickness = 1.0 + z * 2.0;

                                // Debug line drawing
                                // if i == 0 {
                                //     eprintln!("Drawing line: ({:.0},{:.0})->({:.0},{:.0}) thickness={:.1} brightness={}",
                                //         x1, y1, x2, y2, thickness, front_brightness);
                                // }

                                gpu_renderer.add_command(RenderCommand::Line {
                                    x1,
                                    y1,
                                    x2,
                                    y2,
                                    thickness: thickness as f64,
                                    color: (255, front_brightness, front_brightness, front_brightness), // ABGR format - full alpha
                                });
                            }
                        } else {
                            // Normal star (not accelerating) - make sure it's in bounds
                            let sx = self.star_x[i] as f64;
                            let sy = self.star_y[i] as f64;
                            if sx >= rx - size && sx <= rx + rw + size && sy >= ry - size && sy <= ry + rh + size {
                                gpu_renderer.add_command(RenderCommand::Rect {
                                    texture_id: *atlas_id,
                                    dest_x: sx - size / 2.0,
                                    dest_y: sy - size / 2.0,
                                    dest_width: size,
                                    dest_height: size,
                                    rotation: 0.0,
                                    color: (255, brightness, brightness, brightness), // ABGR
                                });
                            }
                        }
                    }
                }

                // Debug star visibility
                static mut FRAME_COUNT: u32 = 0;
                unsafe {
                    FRAME_COUNT += 1;
                    if FRAME_COUNT % 30 == 0 {
                        eprintln!(
                            "Stars: {} visible, {} streaks rendered (z_vel={:.2})",
                            visible_count, streak_count, self.z_velocity
                        );
                    }
                }

                // Debug: Draw a test line when accelerating
                // if self.z_velocity > 0.1 {
                //     gpu_renderer.add_command(RenderCommand::Line {
                //         x1: rx + 10.0,
                //         y1: ry + 10.0,
                //         x2: rx + 100.0,
                //         y2: ry + 100.0,
                //         thickness: 3.0,
                //         color: (255, 255, 0, 0), // ABGR: red line
                //     });
                //     eprintln!("Added test line at ({:.0},{:.0})", rx + 10.0, ry + 10.0);
                // }

                // Draw speed display
                if let Some(ref mut display) = self.speed_display {
                    display.set_x(rx + 10.0);
                    display.set_y(ry + rh - 20.0);
                    display.generate_commands(gpu_renderer);
                }
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            // GPU only
            let _ = (buffer, buffer_width, buffer_height, pitch);
        }

        fn get_resize_edge(&self, x: f64, y: f64) -> Option<u8> {
            if let Some(rect) = &self.rect {
                let (rx, ry, rw, rh) = rect.bounds();
                let edge_threshold = 10.0;

                let near_left = (x - rx).abs() < edge_threshold;
                let near_right = (x - (rx + rw)).abs() < edge_threshold;
                let near_top = (y - ry).abs() < edge_threshold;
                let near_bottom = (y - (ry + rh)).abs() < edge_threshold;

                match (near_left, near_right, near_top, near_bottom) {
                    (true, false, true, false) => Some(5),  // TopLeft
                    (false, true, true, false) => Some(6),  // TopRight
                    (true, false, false, true) => Some(7),  // BottomLeft
                    (false, true, false, true) => Some(8),  // BottomRight
                    (true, false, false, false) => Some(3), // Left
                    (false, true, false, false) => Some(4), // Right
                    (false, false, true, false) => Some(1), // Top
                    (false, false, false, true) => Some(2), // Bottom
                    _ => None,
                }
            } else {
                None
            }
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) -> bool {
            if let Some(rect) = &self.rect {
                let (rx, ry, rw, rh) = rect.bounds();

                // Check if we're on a resize edge
                if let Some(edge) = self.get_resize_edge(x, y) {
                    self.resize_mode = Some(edge);
                    self.drag_offset = (x, y);
                    return true;
                }

                // Check if we're inside the rect for dragging
                if x >= rx && x <= rx + rw && y >= ry && y <= ry + rh {
                    self.dragging = true;
                    self.drag_offset = (x - rx, y - ry);
                    return true;
                }
            }
            false
        }

        pub fn handle_mouse_up(&mut self, _x: f64, _y: f64) -> bool {
            let was_interacting = self.dragging || self.resize_mode.is_some();
            self.dragging = false;
            self.resize_mode = None;
            was_interacting
        }

        pub fn handle_mouse_move(&mut self, x: f64, y: f64) -> bool {
            if let Some(rect) = &mut self.rect {
                if self.dragging {
                    // Move the rect
                    let new_x = x - self.drag_offset.0;
                    let new_y = y - self.drag_offset.1;
                    let (_, _, w, h) = rect.bounds();
                    rect.resize(new_x, new_y, w, h);

                    // Update star positions to move with the rect
                    let dx = new_x - (x - self.drag_offset.0);
                    let dy = new_y - (y - self.drag_offset.1);
                    for i in 0..self.star_x.len() {
                        self.star_x[i] += dx as f32;
                        self.star_y[i] += dy as f32;
                    }
                    return true;
                } else if let Some(edge) = self.resize_mode {
                    // Resize the rect
                    let (rx, ry, rw, rh) = rect.bounds();
                    let (start_x, start_y) = self.drag_offset;
                    let dx = x - start_x;
                    let dy = y - start_y;

                    let (new_x, new_y, new_w, new_h) = match edge {
                        5 => (rx + dx, ry + dy, rw - dx, rh - dy), // TopLeft
                        6 => (rx, ry + dy, rw + dx, rh - dy),      // TopRight
                        7 => (rx + dx, ry, rw - dx, rh + dy),      // BottomLeft
                        8 => (rx, ry, rw + dx, rh + dy),           // BottomRight
                        3 => (rx + dx, ry, rw - dx, rh),           // Left
                        4 => (rx, ry, rw + dx, rh),                // Right
                        1 => (rx, ry + dy, rw, rh - dy),           // Top
                        2 => (rx, ry, rw, rh + dy),                // Bottom
                        _ => (rx, ry, rw, rh),                     // Should never happen
                    };

                    // Ensure minimum size
                    if new_w > 100.0 && new_h > 100.0 {
                        rect.resize(new_x, new_y, new_w, new_h);
                        self.drag_offset = (x, y);
                    }

                    return true;
                }
            }
            false
        }
    }
});
