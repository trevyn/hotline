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
        initialized: bool,
        dragging: bool,
        resize_mode: Option<u8>, // 0=None, 1=Top, 2=Bottom, 3=Left, 4=Right, 5=TopLeft, 6=TopRight, 7=BottomLeft, 8=BottomRight
        drag_offset: (f64, f64),
        speed_display: Option<TextRenderer>,
        // Visual Effects Parameters
        trail_fade_factor: f32,
        star_size_multiplier: f32,
        z_velocity_curve: f32,
        motion_blur_samples: i32,
        chromatic_aberration: f32,
        vortex_twist_amount: f32,
        bloom_radius: f32,
        streak_taper_ratio: f32,

        // Distribution Parameters
        spawn_pattern: i32, // 0=random, 1=radial, 2=spiral, 3=tunnel
        center_bias: f32,
        star_layers: i32,
        density_falloff: f32,
        angular_spread: f32,
        cluster_factor: f32,

        // Animation Parameters
        pulse_frequency: f32,
        wobble_amount: f32,
        rotation_speed: f32,
        time_dilation: f32,

        // Physics Parameters
        drag_coefficient: f32,
        gravity_strength: f32,
        turbulence_scale: f32,
        max_velocity_cap: f32,
        acceleration_curve: i32, // 0=linear, 1=quadratic, 2=cubic
        param_displays: Vec<TextRenderer>,
        panel_visible: bool,
        selected_param: Option<usize>,
        hovered_param: Option<usize>,
        panel_x: f64,
        panel_width: f64,
        param_start_y: f64,
        param_height: f64,
        dragging_param: bool,
        drag_start_x: f64,
        drag_start_value: f32,
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
            self.acceleration_multiplier = 2.0; // Lower for more gradual effect
            self.initialized = true;
            self.dragging = false;
            self.resize_mode = None;
            self.drag_offset = (0.0, 0.0);
            // Visual Effects defaults
            self.trail_fade_factor = 0.8;
            self.star_size_multiplier = 1.0;
            self.z_velocity_curve = 1.0;
            self.motion_blur_samples = 1;
            self.chromatic_aberration = 0.0;
            self.vortex_twist_amount = 0.0;
            self.bloom_radius = 0.0;
            self.streak_taper_ratio = 0.5;

            // Distribution defaults
            self.spawn_pattern = 1; // radial
            self.center_bias = 1.0;
            self.star_layers = 3;
            self.density_falloff = 1.0;
            self.angular_spread = 180.0;
            self.cluster_factor = 0.0;

            // Animation defaults
            self.pulse_frequency = 0.0;
            self.wobble_amount = 0.0;
            self.rotation_speed = 0.0;
            self.time_dilation = 1.0;

            // Physics defaults
            self.drag_coefficient = 0.0;
            self.gravity_strength = 0.0;
            self.turbulence_scale = 0.0;
            self.max_velocity_cap = 100.0;
            self.acceleration_curve = 0; // linear
            self.panel_visible = true;
            self.selected_param = None;
            self.hovered_param = None;
            self.panel_width = 300.0;
            self.param_height = 20.0;
            self.param_start_y = 60.0;
            self.dragging_param = false;

            // Initialize speed display
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }
            let mut display = TextRenderer::new();
            display.set_text(format!("Speed: {:.1}x", self.acceleration_multiplier));
            display.set_color((255, 255, 255, 255));
            self.speed_display = Some(display);

            // Initialize parameter displays
            self.param_displays.clear();

            // Title
            let mut title = TextRenderer::new();
            title.set_text("=== STARFIELD PARAMS ===".to_string());
            title.set_color((255, 255, 255, 255));
            self.param_displays.push(title);

            // Randomize button
            let mut randomize = TextRenderer::new();
            randomize.set_text("[R] Randomize All".to_string());
            randomize.set_color((180, 180, 255, 255));
            self.param_displays.push(randomize);

            // Visual Effects header
            let mut visual_header = TextRenderer::new();
            visual_header.set_text("-- Visual Effects --".to_string());
            visual_header.set_color((255, 200, 200, 255));
            self.param_displays.push(visual_header);

            // Create displays for each parameter
            let param_names = [
                ("trail_fade", "Trail Fade Factor"),
                ("star_size", "Star Size Mult"),
                ("z_curve", "Z Velocity Curve"),
                ("blur_samples", "Motion Blur Samples"),
                ("chromatic", "Chromatic Aberration"),
                ("vortex", "Vortex Twist"),
                ("bloom", "Bloom Radius"),
                ("taper", "Streak Taper Ratio"),
                ("", "-- Distribution --"),
                ("pattern", "Spawn Pattern"),
                ("center_bias", "Center Bias"),
                ("layers", "Star Layers"),
                ("density", "Density Falloff"),
                ("spread", "Angular Spread"),
                ("cluster", "Cluster Factor"),
                ("", "-- Animation --"),
                ("pulse", "Pulse Frequency"),
                ("wobble", "Wobble Amount"),
                ("rotation", "Rotation Speed"),
                ("dilation", "Time Dilation"),
                ("afterimage", "Afterimage Count"),
                ("", "-- Physics --"),
                ("drag", "Drag Coefficient"),
                ("gravity", "Gravity Strength"),
                ("turbulence", "Turbulence Scale"),
                ("max_vel", "Max Velocity Cap"),
                ("accel_curve", "Acceleration Curve"),
            ];

            for (_, name) in param_names.iter() {
                let mut param_display = TextRenderer::new();
                param_display.set_text(name.to_string());
                param_display.set_color((200, 200, 200, 255));
                self.param_displays.push(param_display);
            }
        }

        pub fn set_rect(&mut self, rect: Rect) {
            let (x, y, w, h) = rect.bounds();
            self.rect = Some(rect);
            self.panel_x = x + w - self.panel_width - 10.0;

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

        pub fn randomize_params(&mut self) {
            let mut rng = rand::rng();

            // Visual Effects
            self.trail_fade_factor = rng.random_range(0.1..1.0);
            self.star_size_multiplier = rng.random_range(0.5..5.0);
            self.z_velocity_curve = rng.random_range(0.5..2.0);
            self.motion_blur_samples = rng.random_range(1..10);
            self.chromatic_aberration = rng.random_range(0.0..10.0);
            self.vortex_twist_amount = rng.random_range(0.0..1.0);
            self.bloom_radius = rng.random_range(0.0..20.0);
            self.streak_taper_ratio = rng.random_range(0.1..1.0);

            // Distribution
            self.spawn_pattern = rng.random_range(0..4);
            self.center_bias = rng.random_range(0.0..2.0);
            self.star_layers = rng.random_range(1..10);
            self.density_falloff = rng.random_range(0.1..5.0);
            self.angular_spread = rng.random_range(0.0..180.0);
            self.cluster_factor = rng.random_range(0.0..1.0);

            // Animation
            self.pulse_frequency = rng.random_range(0.0..10.0);
            self.wobble_amount = rng.random_range(0.0..5.0);
            self.rotation_speed = rng.random_range(-10.0..10.0);
            self.time_dilation = rng.random_range(0.1..5.0);

            // Physics
            self.drag_coefficient = rng.random_range(0.0..1.0);
            self.gravity_strength = rng.random_range(0.0..1.0);
            self.turbulence_scale = rng.random_range(0.0..10.0);
            self.max_velocity_cap = rng.random_range(10.0..1000.0);
            self.acceleration_curve = rng.random_range(0..3);

            eprintln!("Randomized starfield parameters!");
        }

        pub fn toggle_panel(&mut self) {
            self.panel_visible = !self.panel_visible;
        }

        fn get_param_value(&self, index: usize) -> Option<f32> {
            match index {
                0 => Some(self.trail_fade_factor),
                1 => Some(self.star_size_multiplier),
                2 => Some(self.z_velocity_curve),
                3 => Some(self.motion_blur_samples as f32),
                4 => Some(self.chromatic_aberration),
                5 => Some(self.vortex_twist_amount),
                6 => Some(self.bloom_radius),
                7 => Some(self.streak_taper_ratio),
                8 => Some(self.spawn_pattern as f32),
                9 => Some(self.center_bias),
                10 => Some(self.star_layers as f32),
                11 => Some(self.density_falloff),
                12 => Some(self.angular_spread),
                13 => Some(self.cluster_factor),
                14 => Some(self.pulse_frequency),
                15 => Some(self.wobble_amount),
                16 => Some(self.rotation_speed),
                17 => Some(self.time_dilation),
                18 => Some(self.drag_coefficient),
                19 => Some(self.gravity_strength),
                20 => Some(self.turbulence_scale),
                21 => Some(self.max_velocity_cap),
                22 => Some(self.acceleration_curve as f32),
                _ => None,
            }
        }

        fn set_param_value(&mut self, index: usize, value: f32) {
            match index {
                0 => self.trail_fade_factor = value.clamp(0.1, 1.0),
                1 => self.star_size_multiplier = value.clamp(0.5, 5.0),
                2 => self.z_velocity_curve = value.clamp(0.5, 2.0),
                3 => self.motion_blur_samples = value.clamp(1.0, 10.0) as i32,
                4 => self.chromatic_aberration = value.clamp(0.0, 10.0),
                5 => self.vortex_twist_amount = value.clamp(0.0, 1.0),
                6 => self.bloom_radius = value.clamp(0.0, 20.0),
                7 => self.streak_taper_ratio = value.clamp(0.1, 1.0),
                8 => self.spawn_pattern = value.clamp(0.0, 3.0) as i32,
                9 => self.center_bias = value.clamp(0.0, 2.0),
                10 => self.star_layers = value.clamp(1.0, 10.0) as i32,
                11 => self.density_falloff = value.clamp(0.1, 5.0),
                12 => self.angular_spread = value.clamp(0.0, 180.0),
                13 => self.cluster_factor = value.clamp(0.0, 1.0),
                14 => self.pulse_frequency = value.clamp(0.0, 10.0),
                15 => self.wobble_amount = value.clamp(0.0, 5.0),
                16 => self.rotation_speed = value.clamp(-10.0, 10.0),
                17 => self.time_dilation = value.clamp(0.1, 5.0),
                18 => self.drag_coefficient = value.clamp(0.0, 1.0),
                19 => self.gravity_strength = value.clamp(0.0, 1.0),
                20 => self.turbulence_scale = value.clamp(0.0, 10.0),
                21 => self.max_velocity_cap = value.clamp(10.0, 1000.0),
                22 => self.acceleration_curve = value.clamp(0.0, 2.0) as i32,
                _ => {}
            }
        }

        fn get_param_range(&self, index: usize) -> Option<(f32, f32)> {
            match index {
                0 => Some((0.1, 1.0)),
                1 => Some((0.5, 5.0)),
                2 => Some((0.5, 2.0)),
                3 => Some((1.0, 10.0)),
                4 => Some((0.0, 10.0)),
                5 => Some((0.0, 1.0)),
                6 => Some((0.0, 20.0)),
                7 => Some((0.1, 1.0)),
                8 => Some((0.0, 3.0)),
                9 => Some((0.0, 2.0)),
                10 => Some((1.0, 10.0)),
                11 => Some((0.1, 5.0)),
                12 => Some((0.0, 180.0)),
                13 => Some((0.0, 1.0)),
                14 => Some((0.0, 10.0)),
                15 => Some((0.0, 5.0)),
                16 => Some((-10.0, 10.0)),
                17 => Some((0.1, 5.0)),
                18 => Some((0.0, 1.0)),
                19 => Some((0.0, 1.0)),
                20 => Some((0.0, 10.0)),
                21 => Some((10.0, 1000.0)),
                22 => Some((0.0, 2.0)),
                _ => None,
            }
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

                            // Use spawn pattern parameter
                            match self.spawn_pattern {
                                0 => {
                                    // Random
                                    self.star_x[i] = rng.random_range(x..x + w) as f32;
                                    self.star_y[i] = rng.random_range(y..y + h) as f32;
                                }
                                1 => {
                                    // Radial
                                    let angle = rng.random_range(0.0..std::f32::consts::TAU);
                                    let angle_range = self.angular_spread.to_radians();
                                    let angle = if angle_range < std::f32::consts::TAU {
                                        rng.random_range(-angle_range / 2.0..angle_range / 2.0)
                                    } else {
                                        angle
                                    };

                                    // Apply center bias and density falloff
                                    let u: f32 = rng.random_range(0.0..1.0);
                                    let max_radius = (w.min(h) / 2.0) as f32;
                                    let radius = if self.center_bias > 0.0 {
                                        -max_radius * (1.0_f32 - u).ln().powf(self.center_bias)
                                    } else {
                                        u * max_radius
                                    };
                                    let radius = radius.min(max_radius * 0.9) * self.density_falloff;

                                    self.star_x[i] = center_x as f32 + angle.cos() * radius;
                                    self.star_y[i] = center_y as f32 + angle.sin() * radius;
                                }
                                2 => {
                                    // Spiral
                                    let t = rng.random_range(0.0..10.0_f32);
                                    let spiral_tightness = 0.3;
                                    let radius = t * (w.min(h) as f32 / 20.0);
                                    let angle = t * spiral_tightness
                                        + (i as f32 % self.star_layers as f32)
                                            * (std::f32::consts::TAU / self.star_layers as f32);

                                    self.star_x[i] = center_x as f32 + angle.cos() * radius;
                                    self.star_y[i] = center_y as f32 + angle.sin() * radius;
                                }
                                3 => {
                                    // Tunnel
                                    let layer = i % self.star_layers.max(1) as usize;
                                    let layer_radius =
                                        (layer as f32 + 1.0) / self.star_layers as f32 * (w.min(h) / 2.0) as f32;
                                    let angle = rng.random_range(0.0..std::f32::consts::TAU);

                                    // Add some randomness for natural look
                                    let radius_variation = rng.random_range(0.8..1.2);
                                    let final_radius = layer_radius * radius_variation * self.density_falloff;

                                    self.star_x[i] = center_x as f32 + angle.cos() * final_radius;
                                    self.star_y[i] = center_y as f32 + angle.sin() * final_radius;
                                }
                                _ => {
                                    self.star_x[i] = rng.random_range(x..x + w) as f32;
                                    self.star_y[i] = rng.random_range(y..y + h) as f32;
                                }
                            }

                            // Apply cluster factor
                            if self.cluster_factor > 0.0 && i > 0 {
                                let cluster_chance = rng.random_range(0.0..1.0);
                                if cluster_chance < self.cluster_factor {
                                    // Cluster near previous star
                                    let cluster_dist = rng.random_range(5.0..30.0);
                                    let cluster_angle = rng.random_range(0.0..std::f32::consts::TAU);
                                    self.star_x[i] = self.star_x[i - 1] + cluster_angle.cos() * cluster_dist;
                                    self.star_y[i] = self.star_y[i - 1] + cluster_angle.sin() * cluster_dist;
                                }
                            }
                        }

                        // Apply physics parameters
                        let dx = self.star_x[i] - center_x as f32;
                        let dy = self.star_y[i] - center_y as f32;
                        let dist = (dx * dx + dy * dy).sqrt();

                        if dist > 0.01 {
                            // Normalize direction
                            let ndx = dx / dist;
                            let ndy = dy / dist;

                            // Apply acceleration curve
                            let accel_factor = match self.acceleration_curve {
                                0 => self.z_velocity,                                     // Linear
                                1 => self.z_velocity * self.z_velocity,                   // Quadratic
                                2 => self.z_velocity * self.z_velocity * self.z_velocity, // Cubic
                                _ => self.z_velocity,
                            };

                            // Base speed with max velocity cap
                            let base_speed = accel_factor * self.star_z[i] * 20.0 * 0.016;
                            let speed = base_speed.min(self.max_velocity_cap);

                            // Apply drag
                            let drag_adjusted_speed = speed * (1.0 - self.drag_coefficient * 0.016);

                            // Add turbulence
                            let mut turbulence_x = 0.0;
                            let mut turbulence_y = 0.0;
                            if self.turbulence_scale > 0.0 {
                                let mut rng = rand::rng();
                                turbulence_x = rng.random_range(-1.0..1.0) * self.turbulence_scale;
                                turbulence_y = rng.random_range(-1.0..1.0) * self.turbulence_scale;
                            }

                            // Apply gravity towards center
                            let gravity_x = if self.gravity_strength > 0.0 && dist > 50.0 {
                                -ndx * self.gravity_strength * 0.016 / (dist / 100.0)
                            } else {
                                0.0
                            };
                            let gravity_y = if self.gravity_strength > 0.0 && dist > 50.0 {
                                -ndy * self.gravity_strength * 0.016 / (dist / 100.0)
                            } else {
                                0.0
                            };

                            // Update position with all forces
                            self.star_x[i] += ndx * drag_adjusted_speed + turbulence_x + gravity_x;
                            self.star_y[i] += ndy * drag_adjusted_speed + turbulence_y + gravity_y;
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

                        // Use same spawn pattern as above
                        match self.spawn_pattern {
                            0 => {
                                // Random
                                self.star_x[i] = rng.random_range(x..x + w) as f32;
                                self.star_y[i] = rng.random_range(y..y + h) as f32;
                            }
                            1 => {
                                // Radial
                                let angle = rng.random_range(0.0..std::f32::consts::TAU);
                                let angle_range = self.angular_spread.to_radians();
                                let angle = if angle_range < std::f32::consts::TAU {
                                    rng.random_range(-angle_range / 2.0..angle_range / 2.0)
                                } else {
                                    angle
                                };

                                let u: f32 = rng.random_range(0.0..1.0);
                                let max_radius = (w.min(h) / 2.0) as f32;
                                let radius = if self.center_bias > 0.0 {
                                    -max_radius * (1.0_f32 - u).ln().powf(self.center_bias)
                                } else {
                                    u * max_radius
                                };
                                let radius = radius.min(max_radius * 0.9) * self.density_falloff;

                                self.star_x[i] = center_x as f32 + angle.cos() * radius;
                                self.star_y[i] = center_y as f32 + angle.sin() * radius;
                            }
                            2 => {
                                // Spiral
                                let t = rng.random_range(0.0..10.0_f32);
                                let spiral_tightness = 0.3;
                                let radius = t * (w.min(h) as f32 / 20.0);
                                let angle = t * spiral_tightness
                                    + (i as f32 % self.star_layers as f32)
                                        * (std::f32::consts::TAU / self.star_layers as f32);

                                self.star_x[i] = center_x as f32 + angle.cos() * radius;
                                self.star_y[i] = center_y as f32 + angle.sin() * radius;
                            }
                            3 => {
                                // Tunnel
                                let layer = i % self.star_layers.max(1) as usize;
                                let layer_radius =
                                    (layer as f32 + 1.0) / self.star_layers as f32 * (w.min(h) / 2.0) as f32;
                                let angle = rng.random_range(0.0..std::f32::consts::TAU);

                                let radius_variation = rng.random_range(0.8..1.2);
                                let final_radius = layer_radius * radius_variation * self.density_falloff;

                                self.star_x[i] = center_x as f32 + angle.cos() * final_radius;
                                self.star_y[i] = center_y as f32 + angle.sin() * final_radius;
                            }
                            _ => {
                                self.star_x[i] = rng.random_range(x..x + w) as f32;
                                self.star_y[i] = rng.random_range(y..y + h) as f32;
                            }
                        }

                        // Apply cluster factor
                        if self.cluster_factor > 0.0 && i > 0 {
                            let cluster_chance = rng.random_range(0.0..1.0);
                            if cluster_chance < self.cluster_factor {
                                let cluster_dist = rng.random_range(5.0..30.0);
                                let cluster_angle = rng.random_range(0.0..std::f32::consts::TAU);
                                self.star_x[i] = self.star_x[i - 1] + cluster_angle.cos() * cluster_dist;
                                self.star_y[i] = self.star_y[i - 1] + cluster_angle.sin() * cluster_dist;
                            }
                        }
                    }
                }
            }
        }

        pub fn setup_gpu_rendering(&mut self, gpu_renderer: &mut dyn ::hotline::GpuRenderingContext) {
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

                    let id = gpu_renderer
                        .create_rgba_texture(&texture_data, texture_size as u32, texture_size as u32)
                        .unwrap();
                    self.atlas_ids.push(Some(id));
                }
            }

            // TODO: Update TextRenderer to use new GPU API
            // if let Some(ref mut display) = self.speed_display {
            //     display.register_atlas(gpu_renderer);
            // }
            // for display in &mut self.param_displays {
            //     display.register_atlas(gpu_renderer);
            // }
        }

        pub fn render_gpu(&mut self, gpu_renderer: &mut dyn ::hotline::GpuRenderingContext) {
            // Early exit if rect not set or no stars initialized
            if self.rect.is_none() || self.star_x.is_empty() {
                static mut LOGGED: bool = false;
                unsafe {
                    if !LOGGED {
                        eprintln!(
                            "Starfield: render_gpu called but not initialized (rect={}, stars={})",
                            self.rect.is_some(),
                            self.star_x.len()
                        );
                        LOGGED = true;
                    }
                }
                return;
            }

            // Update stars
            self.update(0.016);

            // Make sure atlases are registered
            if self.atlas_ids.is_empty() {
                self.setup_gpu_rendering(gpu_renderer);
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
                    gpu_renderer.add_textured_rect(
                        rx as f32,
                        ry as f32,
                        rw as f32,
                        rh as f32,
                        atlas_id,
                        [0.0, 0.0, 0.0, 1.0], // Black
                    );
                }

                // Draw border
                let border_width = 2.0f32;
                let border_color = [100.0 / 255.0, 100.0 / 255.0, 255.0 / 255.0, 1.0]; // Light blue

                // Top border
                gpu_renderer.add_solid_rect(rx as f32, ry as f32, rw as f32, border_width, border_color);

                // Bottom border
                gpu_renderer.add_solid_rect(
                    rx as f32,
                    (ry + rh - border_width as f64) as f32,
                    rw as f32,
                    border_width,
                    border_color,
                );

                // Left border
                gpu_renderer.add_solid_rect(rx as f32, ry as f32, border_width, rh as f32, border_color);

                // Right border
                gpu_renderer.add_solid_rect(
                    (rx + rw - border_width as f64) as f32,
                    ry as f32,
                    border_width,
                    rh as f32,
                    border_color,
                );

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

                        // Skip stars that are at or very near the origin (likely uninitialized)
                        if star_x.abs() < 1.0 && star_y.abs() < 1.0 {
                            continue;
                        }

                        // Check if star is within visible bounds
                        if star_x >= rx && star_x <= rx + rw && star_y >= ry && star_y <= ry + rh {
                            visible_count += 1;
                        }

                        // Apply visual parameters to star rendering
                        let center_x = rx + rw / 2.0;
                        let center_y = ry + rh / 2.0;
                        let dx = star_x - center_x;
                        let dy = star_y - center_y;
                        let dist = (dx * dx + dy * dy).sqrt();

                        // Apply animation effects
                        let time =
                            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f32()
                                * self.time_dilation.max(0.1);

                        // Pulse effect
                        let pulse = if self.pulse_frequency > 0.0 {
                            1.0 + (time * self.pulse_frequency).sin() * 0.2
                        } else {
                            1.0
                        };

                        // Wobble effect
                        let wobble_offset_x = if self.wobble_amount > 0.0 {
                            (time * 3.0 + i as f32 * 0.5).sin() * self.wobble_amount
                        } else {
                            0.0
                        };
                        let wobble_offset_y = if self.wobble_amount > 0.0 {
                            (time * 2.7 + i as f32 * 0.7).cos() * self.wobble_amount
                        } else {
                            0.0
                        };

                        // Apply rotation
                        let rotation_angle = if self.rotation_speed > 0.0 { time * self.rotation_speed } else { 0.0 };

                        // Vortex twist effect
                        let twist_angle = if self.vortex_twist_amount > 0.0 && dist > 0.01 {
                            ((dist / 100.0) * self.vortex_twist_amount as f64) as f32
                        } else {
                            0.0
                        };

                        // Apply rotation and twist
                        let total_angle = rotation_angle + twist_angle;
                        let cos_a = total_angle.cos() as f64;
                        let sin_a = total_angle.sin() as f64;
                        let rotated_x = dx * cos_a - dy * sin_a;
                        let rotated_y = dx * sin_a + dy * cos_a;
                        let final_x = center_x + rotated_x + wobble_offset_x as f64;
                        let final_y = center_y + rotated_y + wobble_offset_y as f64;

                        // Skip stars whose final position is near origin (likely due to transformation errors)
                        if final_x.abs() < 50.0 && final_y.abs() < 50.0 {
                            static mut ORIGIN_DEBUG: u32 = 0;
                            unsafe {
                                if ORIGIN_DEBUG < 5 {
                                    eprintln!("Star transformed near origin: final=({:.1}, {:.1}), original=({:.1}, {:.1}), center=({:.1}, {:.1})", 
                                        final_x, final_y, star_x, star_y, center_x, center_y);
                                    ORIGIN_DEBUG += 1;
                                }
                            }
                            continue;
                        }

                        // Size with star_size_multiplier and pulse
                        let final_size = size * self.star_size_multiplier as f64 * pulse as f64;

                        // Brightness with bloom effect
                        let bloom_brightness = if self.bloom_radius > 0.0 {
                            ((brightness as f32) * (1.0 + self.bloom_radius * 0.5)).min(255.0) as u8
                        } else {
                            brightness
                        };

                        // Gradual transition from dots to streaks based on z_velocity
                        let z_vel_adjusted = self.z_velocity.powf(self.z_velocity_curve);
                        let streak_factor = (z_vel_adjusted * 5.0).min(1.0); // 0 to 1 transition over 0.0 to 0.2 velocity

                        if z_vel_adjusted > 0.01 && streak_factor > 0.1 {
                            streak_count += 1;

                            if dist < 5.0 {
                                // Draw motion blur samples for close stars
                                for j in 0..self.motion_blur_samples.max(1) {
                                    let blur_offset = j as f64 / self.motion_blur_samples.max(1) as f64;
                                    let blur_alpha = ((255.0 * (1.0 - blur_offset * 0.7)) as u8).min(bloom_brightness);

                                    let brightness_f = bloom_brightness as f32 / 255.0;
                                    let alpha_f = blur_alpha as f32 / 255.0;
                                    if let Some(Some(atlas_id)) = self.atlas_ids.get(atlas_idx) {
                                        let brightness_f = bloom_brightness as f32 / 255.0;
                                        let alpha_f = blur_alpha as f32 / 255.0;
                                        gpu_renderer.add_textured_rect(
                                            (final_x - final_size / 2.0 - wobble_offset_x as f64 * blur_offset) as f32,
                                            (final_y - final_size / 2.0 - wobble_offset_y as f64 * blur_offset) as f32,
                                            final_size as f32,
                                            final_size as f32,
                                            *atlas_id,
                                            [brightness_f, brightness_f, brightness_f, alpha_f],
                                        );
                                    }
                                }
                            } else {
                                // Streak length with trail_fade_factor and gradual transition
                                let base_streak = 2.0; // Shorter base for smoother transition
                                let streak_length = (base_streak
                                    + (z_vel_adjusted * z * 100.0 * self.trail_fade_factor) as f64)
                                    * streak_factor as f64;

                                // Normalize direction (avoid division by zero)
                                let ndx = if dist > 0.1 { dx / dist } else { 0.0 };
                                let ndy = if dist > 0.1 { dy / dist } else { 0.0 };

                                // Skip if direction is invalid OR star is too close to center
                                if (ndx == 0.0 && ndy == 0.0) || dist < 5.0 {
                                    continue;
                                }

                                // Debug stars near center that might cause origin lines
                                if dist < 50.0 {
                                    static mut CENTER_DEBUG: u32 = 0;
                                    unsafe {
                                        if CENTER_DEBUG < 5 {
                                            eprintln!("Star near center: star_pos=({:.1}, {:.1}), center=({:.1}, {:.1}), dist={:.1}, dx={:.1}, dy={:.1}", 
                                                star_x, star_y, center_x, center_y, dist, dx, dy);
                                            CENTER_DEBUG += 1;
                                        }
                                    }
                                }

                                // Apply chromatic aberration to streaks
                                for chroma_idx in 0..if self.chromatic_aberration > 0.0 { 3 } else { 1 } {
                                    let chroma_offset = (chroma_idx as f32 - 1.0) * self.chromatic_aberration * 2.0;
                                    let chroma_x1 = final_x - ndx * streak_length + chroma_offset as f64 * ndy;
                                    let chroma_y1 = final_y - ndy * streak_length - chroma_offset as f64 * ndx;
                                    let chroma_x2 = final_x + chroma_offset as f64 * ndy;
                                    let chroma_y2 = final_y - chroma_offset as f64 * ndx;

                                    // Color based on chromatic channel
                                    let (r, g, b) = match chroma_idx {
                                        0 => (bloom_brightness, 0, 0),
                                        1 => (0, bloom_brightness, 0),
                                        2 => (0, 0, bloom_brightness),
                                        _ => (bloom_brightness, bloom_brightness, bloom_brightness),
                                    };

                                    // Thickness with taper
                                    let thickness = (1.0 + z * 2.0) * (1.0 - self.streak_taper_ratio * 0.5);

                                    // Draw main line only
                                    let r_f = r as f32 / 255.0;
                                    let g_f = g as f32 / 255.0;
                                    let b_f = b as f32 / 255.0;
                                    let alpha_f = bloom_brightness as f32 / 255.0;

                                    // Validate line coordinates to prevent spurious lines from origin
                                    let x1 = chroma_x1 as f32;
                                    let y1 = chroma_y1 as f32;
                                    let x2 = chroma_x2 as f32;
                                    let y2 = chroma_y2 as f32;

                                    // Only draw if coordinates are valid and within reasonable bounds
                                    if x1.is_finite() && y1.is_finite() && x2.is_finite() && y2.is_finite() {
                                        // Skip lines near origin (likely uninitialized or invalid)
                                        if (x1.abs() < 10.0 && y1.abs() < 10.0) || (x2.abs() < 10.0 && y2.abs() < 10.0)
                                        {
                                            continue;
                                        }

                                        // Also check that the line isn't degenerate (same start and end)
                                        let dx = x2 - x1;
                                        let dy = y2 - y1;
                                        if dx.abs() > 0.001 || dy.abs() > 0.001 {
                                            // Bounds check - skip lines that go way outside viewport
                                            if x1.abs() < 10000.0
                                                && y1.abs() < 10000.0
                                                && x2.abs() < 10000.0
                                                && y2.abs() < 10000.0
                                            {
                                                // Debug spurious lines
                                                static mut DEBUG_COUNT: u32 = 0;
                                                unsafe {
                                                    if DEBUG_COUNT < 5 {
                                                        eprintln!("Starfield line: ({:.1}, {:.1}) to ({:.1}, {:.1}), star_pos=({:.1}, {:.1}), dist={:.1}, ndx={:.3}, ndy={:.3}, streak_len={:.1}", 
                                                            x1, y1, x2, y2, star_x, star_y, dist, ndx, ndy, streak_length);
                                                        DEBUG_COUNT += 1;
                                                    }
                                                }
                                                gpu_renderer.add_line(
                                                    x1,
                                                    y1,
                                                    x2,
                                                    y2,
                                                    thickness,
                                                    [r_f, g_f, b_f, alpha_f],
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Always draw the star dot (fade it based on streak_factor)
                        if final_x >= rx - final_size
                            && final_x <= rx + rw + final_size
                            && final_y >= ry - final_size
                            && final_y <= ry + rh + final_size
                        {
                            // Fade the dot as streaks get stronger
                            let dot_alpha = ((1.0 - streak_factor * 0.7) * 255.0) as u8;

                            // Draw bloom effect
                            if self.bloom_radius > 0.0 {
                                let bloom_size = final_size * (1.0 + self.bloom_radius as f64);
                                let bloom_alpha = ((64.0 * (1.0 - streak_factor * 0.5)) as u8).min(64);
                                let bloom_brightness_f = (bloom_brightness / 2) as f32 / 255.0;
                                let bloom_alpha_f = bloom_alpha as f32 / 255.0;
                                if let Some(Some(atlas_id)) = self.atlas_ids.get(atlas_idx) {
                                    let bloom_brightness_f = (bloom_brightness / 2) as f32 / 255.0;
                                    let bloom_alpha_f = bloom_alpha as f32 / 255.0;
                                    gpu_renderer.add_textured_rect(
                                        (final_x - bloom_size / 2.0) as f32,
                                        (final_y - bloom_size / 2.0) as f32,
                                        bloom_size as f32,
                                        bloom_size as f32,
                                        *atlas_id,
                                        [bloom_brightness_f, bloom_brightness_f, bloom_brightness_f, bloom_alpha_f],
                                    );
                                }
                            }

                            // Draw main star
                            let brightness_f = bloom_brightness as f32 / 255.0;
                            let alpha_f = dot_alpha as f32 / 255.0;
                            if let Some(Some(atlas_id)) = self.atlas_ids.get(atlas_idx) {
                                let brightness_f = bloom_brightness as f32 / 255.0;
                                let alpha_f = dot_alpha as f32 / 255.0;
                                gpu_renderer.add_textured_rect(
                                    (final_x - final_size / 2.0) as f32,
                                    (final_y - final_size / 2.0) as f32,
                                    final_size as f32,
                                    final_size as f32,
                                    *atlas_id,
                                    [brightness_f, brightness_f, brightness_f, alpha_f],
                                );
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
                    display.render_gpu(gpu_renderer);
                }

                // Draw parameter panel
                if self.panel_visible {
                    let panel_y = ry + 10.0;

                    // Draw panel background
                    gpu_renderer.add_solid_rect(
                        self.panel_x as f32,
                        panel_y as f32,
                        self.panel_width as f32,
                        (rh - 20.0) as f32,
                        [0.156, 0.156, 0.156, 0.784], // Semi-transparent dark background
                    );

                    // Draw panel border
                    // Left border
                    gpu_renderer.add_solid_rect(
                        self.panel_x as f32,
                        panel_y as f32,
                        1.0,
                        (rh - 20.0) as f32,
                        [0.5, 0.5, 0.5, 1.0], // Gray border
                    );

                    // Update and draw parameter displays
                    let mut y_offset = panel_y + 10.0;
                    let param_indices = [
                        (None, 0),      // Title
                        (None, 1),      // Randomize button
                        (None, 2),      // Visual header
                        (Some(0), 3),   // trail_fade
                        (Some(1), 4),   // star_size
                        (Some(2), 5),   // z_curve
                        (Some(3), 6),   // blur_samples
                        (Some(4), 7),   // chromatic
                        (Some(5), 8),   // vortex
                        (Some(6), 9),   // bloom
                        (Some(7), 10),  // taper
                        (None, 11),     // Distribution header
                        (Some(8), 12),  // pattern
                        (Some(9), 13),  // center_bias
                        (Some(10), 14), // layers
                        (Some(11), 15), // density
                        (Some(12), 16), // spread
                        (Some(13), 17), // cluster
                        (None, 18),     // Animation header
                        (Some(14), 19), // pulse
                        (Some(15), 20), // wobble
                        (Some(16), 21), // rotation
                        (Some(17), 22), // dilation
                        (None, 23),     // Physics header
                        (Some(18), 24), // drag
                        (Some(19), 25), // gravity
                        (Some(20), 26), // turbulence
                        (Some(21), 27), // max_vel
                        (Some(22), 28), // accel_curve
                    ];

                    // First, collect all the data we need
                    let mut display_updates = Vec::new();
                    let param_names = [
                        "Trail Fade",
                        "Star Size",
                        "Z Curve",
                        "Blur Samples",
                        "Chromatic",
                        "Vortex",
                        "Bloom",
                        "Taper",
                        "Pattern",
                        "Center Bias",
                        "Layers",
                        "Density",
                        "Spread",
                        "Cluster",
                        "Pulse",
                        "Wobble",
                        "Rotation",
                        "Dilation",
                        "Drag",
                        "Gravity",
                        "Turbulence",
                        "Max Vel",
                        "Accel Curve",
                    ];

                    for (param_idx, display_idx) in param_indices.iter() {
                        let text_and_color = if let Some(idx) = param_idx {
                            if let Some(value) = self.get_param_value(*idx) {
                                let name = param_names.get(*idx).unwrap_or(&"Unknown");

                                // Special formatting for integer parameters
                                let value_str = match idx {
                                    3 | 8 | 10 | 22 => format!("{:.0}", value),
                                    21 => format!("{:.0}", value), // max velocity
                                    _ => format!("{:.2}", value),
                                };

                                let text = format!("{}: {}", name, value_str);

                                // Determine color
                                let color = if self.selected_param == Some(*idx) {
                                    (255, 255, 200, 255)
                                } else if self.hovered_param == Some(*idx) {
                                    (220, 220, 255, 255)
                                } else {
                                    (200, 200, 200, 255)
                                };

                                Some((text, color))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let bar_data = if let Some(idx) = param_idx {
                            if let (Some(value), Some((min, max))) =
                                (self.get_param_value(*idx), self.get_param_range(*idx))
                            {
                                Some((value, min, max))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        display_updates.push((*display_idx, text_and_color, bar_data));
                    }

                    // Now apply the updates
                    for (display_idx, text_and_color, bar_data) in display_updates {
                        if let Some(display) = self.param_displays.get_mut(display_idx) {
                            display.set_x(self.panel_x + 10.0);
                            display.set_y(y_offset);

                            // Apply the text and color
                            if let Some((text, color)) = text_and_color {
                                display.set_text(text);
                                display.set_color(color);
                            }

                            display.render_gpu(gpu_renderer);

                            // Draw value bars
                            if let Some((value, min, max)) = bar_data {
                                let bar_x = self.panel_x + 150.0;
                                let bar_width = 120.0;
                                let bar_height = 10.0;
                                let normalized = (value - min) / (max - min);

                                // Background bar
                                gpu_renderer.add_solid_rect(
                                    bar_x as f32,
                                    (y_offset + 2.0) as f32,
                                    bar_width as f32,
                                    bar_height as f32,
                                    [60.0 / 255.0, 60.0 / 255.0, 60.0 / 255.0, 1.0], // Dark gray
                                );

                                // Value bar
                                gpu_renderer.add_solid_rect(
                                    bar_x as f32,
                                    (y_offset + 2.0) as f32,
                                    (bar_width * normalized as f64) as f32,
                                    bar_height as f32,
                                    [255.0 / 255.0, 105.0 / 255.0, 180.0 / 255.0, 1.0], // Pink
                                );
                            }

                            y_offset += self.param_height;
                        }
                    }
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

                // Check if click is in parameter panel
                if self.panel_visible && x >= self.panel_x && x <= self.panel_x + self.panel_width {
                    let panel_y = ry + 10.0;
                    let relative_y = y - panel_y - 10.0;

                    // Check if clicking on a parameter
                    let param_index = (relative_y / self.param_height) as usize;

                    // Map display index to parameter index
                    let param_map = [
                        None,
                        None,
                        None, // Title, Randomize, Visual header
                        Some(0),
                        Some(1),
                        Some(2),
                        Some(3),
                        Some(4),
                        Some(5),
                        Some(6),
                        Some(7), // Visual params
                        None,    // Distribution header
                        Some(8),
                        Some(9),
                        Some(10),
                        Some(11),
                        Some(12),
                        Some(13), // Distribution params
                        None,     // Animation header
                        Some(14),
                        Some(15),
                        Some(16),
                        Some(17),
                        Some(18), // Animation params
                        None,     // Physics header
                        Some(19),
                        Some(20),
                        Some(21),
                        Some(22),
                        Some(23), // Physics params
                    ];

                    if param_index < param_map.len() {
                        if param_index == 1 {
                            // Clicked on Randomize button
                            self.randomize_params();
                            return true;
                        } else if let Some(idx) = param_map.get(param_index).and_then(|&p| p) {
                            // Clicked on a parameter
                            self.selected_param = Some(idx);
                            self.dragging_param = true;
                            self.drag_start_x = x;
                            if let Some(value) = self.get_param_value(idx) {
                                self.drag_start_value = value;
                            }
                            return true;
                        }
                    }

                    return true;
                }

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
            let was_interacting = self.dragging || self.resize_mode.is_some() || self.dragging_param;
            self.dragging = false;
            self.resize_mode = None;
            self.dragging_param = false;
            was_interacting
        }

        pub fn handle_mouse_move(&mut self, x: f64, y: f64) -> bool {
            if let Some(rect) = &mut self.rect {
                let (_rx, ry, _, _) = rect.bounds();

                // Handle parameter dragging
                if self.dragging_param {
                    if let Some(idx) = self.selected_param {
                        let dx = x - self.drag_start_x;
                        let sensitivity = 0.01;

                        if let Some((min, max)) = self.get_param_range(idx) {
                            let range = max - min;
                            let delta = (dx * sensitivity * range as f64) as f32;
                            let new_value = self.drag_start_value + delta;
                            self.set_param_value(idx, new_value);
                        }
                    }
                    return true;
                }

                // Update hovered parameter
                if self.panel_visible && x >= self.panel_x && x <= self.panel_x + self.panel_width {
                    let panel_y = ry + 10.0;
                    let relative_y = y - panel_y - 10.0;
                    let param_index = (relative_y / self.param_height) as usize;

                    let param_map = [
                        None,
                        None,
                        None,
                        Some(0),
                        Some(1),
                        Some(2),
                        Some(3),
                        Some(4),
                        Some(5),
                        Some(6),
                        Some(7),
                        None,
                        Some(8),
                        Some(9),
                        Some(10),
                        Some(11),
                        Some(12),
                        Some(13),
                        None,
                        Some(14),
                        Some(15),
                        Some(16),
                        Some(17),
                        Some(18),
                        None,
                        Some(19),
                        Some(20),
                        Some(21),
                        Some(22),
                        Some(23),
                    ];

                    if param_index < param_map.len() {
                        self.hovered_param = param_map.get(param_index).and_then(|&p| p);
                    } else {
                        self.hovered_param = None;
                    }
                } else {
                    self.hovered_param = None;
                }

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
