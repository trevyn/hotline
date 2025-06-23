hotline::object!({
    use rand::Rng;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    // 3D star representation
    #[derive(Clone, Copy, Debug, ::hotline::serde::Serialize, ::hotline::serde::Deserialize)]
    #[serde(crate = "::hotline::serde")]
    struct StarData {
        pos: (f32, f32, f32), // World position
        brightness: u8,
        size: f32,
    }

    // Code poster representation
    #[derive(Clone, Debug)]
    struct CodePoster {
        pos: (f32, f32, f32),    // World position
        file_path: PathBuf,      // Path to the source file
        display_name: String,    // Short name to display
        content: Option<String>, // Cached file content
        lines_to_show: usize,    // How many lines to display based on distance
        color: (u8, u8, u8, u8), // RGBA color based on file type
        width: f32,              // Poster width in world units
        height: f32,             // Poster height in world units
    }

    #[derive(Default, Clone)]
    pub struct Starfield {
        // 3D star field
        stars: Vec<StarData>,

        // Code posters
        code_posters: Vec<CodePoster>,
        all_source_files: Vec<PathBuf>, // All discovered source files
        poster_text_renderers: HashMap<usize, Vec<TextRenderer>>, // Poster index -> text renderers

        // Camera state
        camera_pos: (f32, f32, f32),      // Camera position in world space
        camera_velocity: (f32, f32, f32), // Current velocity
        camera_yaw: f32,                  // Rotation around Y axis
        camera_pitch: f32,                // Rotation around X axis

        // Camera basis vectors (calculated from yaw/pitch)
        camera_forward: (f32, f32, f32),
        camera_right: (f32, f32, f32),
        camera_up: (f32, f32, f32),

        // Control inputs
        forward_accel: f32,          // LT - RT (LT forward, RT backward)
        strafe_velocity: (f32, f32), // Left stick X/Y
        six_dof_mode: bool,          // True for space sim, false for FPS-style

        // Rendering
        rect: Option<Rect>,
        atlas_ids: Vec<Option<u32>>,

        // Movement parameters
        acceleration_multiplier: f32,
        strafe_speed: f32,
        max_velocity: f32,
        damping: f32,

        // Visual parameters
        fov: f32, // Field of view in radians
        star_size_base: f32,
        star_brightness_base: f32,
        max_render_distance: f32,
        streak_velocity_threshold: f32,
        streak_length_multiplier: f32,

        // Star field parameters
        star_density: f32,   // Stars per cubic unit
        spawn_radius: f32,   // Radius around camera to spawn stars
        despawn_radius: f32, // Radius beyond which to remove stars

        // Code poster parameters
        poster_spawn_radius: f32,   // Radius around camera to spawn posters
        poster_despawn_radius: f32, // Radius beyond which to remove posters
        poster_density: f32,        // Posters per cubic unit
        max_poster_distance: f32,   // Maximum distance to render text
        poster_scale: f32,          // Base scale for posters

        // UI elements
        speed_display: Option<TextRenderer>,
        param_displays: Vec<TextRenderer>,
        panel_visible: bool,
        selected_param: Option<usize>,
        hovered_param: Option<usize>,
        panel_x: f64,
        panel_width: f64,
        param_height: f64,

        // Interaction state
        dragging: bool,
        resize_mode: Option<u8>,
        drag_offset: (f64, f64),
        dragging_param: bool,
        drag_start_x: f64,
        drag_start_value: f32,

        // Frame timing
        last_update_time: f64,

        // Random state for consistent star generation
        seed: u64,
    }

    impl Starfield {
        // Scan the codebase for source files
        fn scan_source_files(&mut self) {
            self.all_source_files.clear();

            // Scan different directories
            let dirs_to_scan = ["objects", "hotline", "runtime", "hotline-macros"];

            for dir in &dirs_to_scan {
                if let Ok(_entries) = std::fs::read_dir(dir) {
                    self.scan_directory_recursive(&Path::new(dir));
                }
            }

            if self.all_source_files.is_empty() {
                eprintln!("WARNING: No source files found! Searched dirs: {:?}", dirs_to_scan);
            }
        }

        fn scan_directory_recursive(&mut self, dir: &Path) {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        self.scan_directory_recursive(&path);
                    } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                        self.all_source_files.push(path);
                    }
                }
            }
        }

        // Get color based on file path
        fn get_file_color(path: &Path) -> (u8, u8, u8, u8) {
            if let Some(first_component) = path.components().next() {
                match first_component.as_os_str().to_str() {
                    Some("objects") => (100, 200, 255, 255),        // Light blue for objects
                    Some("hotline") => (255, 200, 100, 255),        // Orange for hotline core
                    Some("runtime") => (200, 255, 100, 255),        // Light green for runtime
                    Some("hotline-macros") => (255, 100, 200, 255), // Pink for macros
                    _ => (200, 200, 200, 255),                      // Gray for others
                }
            } else {
                (200, 200, 200, 255)
            }
        }

        pub fn initialize(&mut self) {
            // Clear existing stars and posters
            self.stars.clear();
            self.code_posters.clear();
            self.poster_text_renderers.clear();

            // Initialize camera at origin looking down -Z
            self.camera_pos = (0.0, 0.0, 0.0);
            self.camera_velocity = (0.0, 0.0, 0.0);
            self.camera_yaw = 0.0;
            self.camera_pitch = 0.0;

            // Calculate initial camera basis vectors
            self.update_camera_basis();

            // Control mode
            self.six_dof_mode = true; // Default to 6DOF space movement

            // Movement parameters
            self.acceleration_multiplier = 40.0; // Slightly reduced for better control
            self.strafe_speed = 25.0; // Slightly reduced for better control
            self.max_velocity = 300.0; // Increased for more exciting movement
            self.damping = 0.98; // Less damping for more responsive feel

            // Visual parameters
            self.fov = std::f32::consts::PI / 3.0; // 60 degrees
            self.star_size_base = 2.0;
            self.star_brightness_base = 200.0;
            self.max_render_distance = 1000.0;
            self.streak_velocity_threshold = 50.0;
            self.streak_length_multiplier = 0.5;

            // Star field parameters
            self.star_density = 0.0001; // Stars per cubic unit (reduced for performance)
            self.spawn_radius = 300.0;
            self.despawn_radius = 400.0;

            // Code poster parameters
            self.poster_spawn_radius = 200.0;
            self.poster_despawn_radius = 300.0;
            self.poster_density = 0.0001; // Increased to spawn ~3-4 posters
            self.max_poster_distance = 150.0;
            self.poster_scale = 30.0; // Base size of posters

            // UI state
            self.panel_visible = true;
            self.selected_param = None;
            self.hovered_param = None;
            self.panel_width = 300.0;
            self.param_height = 20.0;
            self.dragging = false;
            self.resize_mode = None;
            self.drag_offset = (0.0, 0.0);
            self.dragging_param = false;

            // Initialize random seed from current time
            self.seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();

            // Set initial time
            self.last_update_time =
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();

            // Initialize speed display
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }
            let mut display = TextRenderer::new();
            display.set_text("Speed: 0.0".to_string());
            display.set_color((255, 255, 255, 255));
            self.speed_display = Some(display);

            // Initialize parameter displays
            self.param_displays.clear();

            // Title
            let mut title = TextRenderer::new();
            title.set_text("=== 3D STARFIELD ===".to_string());
            title.set_color((255, 255, 255, 255));
            self.param_displays.push(title);

            // Camera info
            let mut cam_header = TextRenderer::new();
            cam_header.set_text("-- Camera --".to_string());
            cam_header.set_color((255, 200, 200, 255));
            self.param_displays.push(cam_header);

            // Create displays for new parameters
            let param_names = [
                "Position: (0, 0, 0)",
                "Velocity: 0.0",
                "Yaw: 0.0°",
                "Pitch: 0.0°",
                "",
                "-- Movement --",
                "Mode: 6DOF Space",
                "Acceleration: 40.0",
                "Strafe Speed: 25.0",
                "Max Velocity: 300.0",
                "Damping: 0.98",
                "",
                "-- Visual --",
                "FOV: 60°",
                "Star Size: 2.0",
                "Render Distance: 1000",
                "Streak Threshold: 50.0",
                "",
                "-- Star Field --",
                "Star Count: 0",
                "Density: 0.001",
                "Spawn Radius: 300",
            ];

            for name in param_names.iter() {
                let mut param_display = TextRenderer::new();
                param_display.set_text(name.to_string());
                param_display.set_color((200, 200, 200, 255));
                self.param_displays.push(param_display);
            }

            // Spawn initial stars around origin
            self.spawn_initial_stars();

            // Scan for source files and spawn initial posters
            self.scan_source_files();
            self.spawn_initial_posters();
        }

        pub fn set_rect(&mut self, rect: Rect) {
            let (x, _y, w, _h) = rect.bounds();
            self.rect = Some(rect);
            self.panel_x = x + w - self.panel_width - 10.0;
        }

        // Update camera basis vectors from yaw/pitch
        fn update_camera_basis(&mut self) {
            let yaw = self.camera_yaw;
            let pitch = self.camera_pitch;

            // Calculate forward vector using standard FPS camera math
            // Yaw rotates around Y axis, pitch rotates around X axis
            self.camera_forward = (
                -yaw.sin() * pitch.cos(), // X: -sin(yaw) * cos(pitch)
                pitch.sin(),              // Y: sin(pitch)
                -yaw.cos() * pitch.cos(), // Z: -cos(yaw) * cos(pitch)
            );

            // Right vector is perpendicular to forward in XZ plane
            self.camera_right = (
                yaw.cos(),  // X: cos(yaw)
                0.0,        // Y: 0 (always horizontal)
                -yaw.sin(), // Z: -sin(yaw)
            );

            // Up vector via cross product: up = right × forward
            self.camera_up = (
                self.camera_right.1 * self.camera_forward.2 - self.camera_right.2 * self.camera_forward.1,
                self.camera_right.2 * self.camera_forward.0 - self.camera_right.0 * self.camera_forward.2,
                self.camera_right.0 * self.camera_forward.1 - self.camera_right.1 * self.camera_forward.0,
            );
        }

        // Spawn initial stars in a sphere around origin
        fn spawn_initial_stars(&mut self) {
            let mut rng = rand::rng();
            let volume = (4.0 / 3.0) * std::f32::consts::PI * self.spawn_radius.powi(3);
            let star_count = (volume * self.star_density) as usize;

            for _ in 0..star_count {
                // Random position in sphere
                let theta = rng.random_range(0.0..std::f32::consts::TAU);
                let phi = rng.random_range(0.0..std::f32::consts::PI);
                let r = rng.random_range(0.0..self.spawn_radius);

                let x = r * phi.sin() * theta.cos();
                let y = r * phi.sin() * theta.sin();
                let z = r * phi.cos();

                let brightness = rng.random_range(100..255);
                let size = rng.random_range(0.5..2.0);

                self.stars.push(StarData { pos: (x, y, z), brightness, size });
            }
        }

        // Spawn initial code posters around origin
        fn spawn_initial_posters(&mut self) {
            let mut rng = rand::rng();
            let volume = (4.0 / 3.0) * std::f32::consts::PI * self.poster_spawn_radius.powi(3);
            let poster_count = ((volume * self.poster_density) as usize).min(self.all_source_files.len());

            if poster_count == 0 && !self.all_source_files.is_empty() {
                eprintln!(
                    "WARNING: Poster count is 0! volume={}, density={}, files={}",
                    volume,
                    self.poster_density,
                    self.all_source_files.len()
                );
            }

            // Randomly select files to display
            let mut selected_files: Vec<PathBuf> = Vec::new();
            let mut available_indices: Vec<usize> = (0..self.all_source_files.len()).collect();

            for _ in 0..poster_count {
                if available_indices.is_empty() {
                    break;
                }
                let idx = rng.random_range(0..available_indices.len());
                let file_idx = available_indices.remove(idx);
                selected_files.push(self.all_source_files[file_idx].clone());
            }

            // Create posters for selected files
            for file_path in selected_files.iter() {
                // Spawn in a cone in front of the camera (positive Z region)
                // Use cylindrical coordinates for better distribution
                let angle = rng.random_range(0.0..std::f32::consts::TAU); // Full circle around Z axis
                let radius = rng.random_range(20.0..80.0); // Lateral distance from Z axis
                let z = rng.random_range(40.0..120.0); // Positive Z (in front of camera)

                // Convert to Cartesian
                let x = radius * angle.cos();
                let y = radius * angle.sin();
                // z is already set

                // Add some vertical variation
                let y_offset = rng.random_range(-20.0..20.0);
                let y = y + y_offset;

                // Create display name
                let display_name = file_path.strip_prefix(".").unwrap_or(&file_path).to_string_lossy().to_string();

                let poster = CodePoster {
                    pos: (x, y, z),
                    file_path: file_path.clone(),
                    display_name,
                    content: None,
                    lines_to_show: 0,
                    color: Self::get_file_color(&file_path),
                    width: self.poster_scale,
                    height: self.poster_scale * 1.5,
                };

                self.code_posters.push(poster);
            }

            if self.code_posters.is_empty() && !self.all_source_files.is_empty() {
                eprintln!(
                    "WARNING: No code posters spawned despite {} source files available",
                    self.all_source_files.len()
                );
            }
        }

        pub fn update_controller(
            &mut self,
            left_x: f32,
            left_y: f32,
            right_x: f32,
            right_y: f32,
            left_trigger: f32,
            right_trigger: f32,
        ) {
            // Store control inputs
            self.strafe_velocity = (left_x, left_y);
            self.forward_accel = left_trigger - right_trigger; // Swapped: LT forward, RT backward

            // Update camera rotation from right stick
            let rotation_speed = 0.015; // Doubled for more responsive feel
            self.camera_yaw += right_x * rotation_speed;
            self.camera_pitch -= right_y * rotation_speed; // Invert for intuitive control

            // Clamp pitch to prevent gimbal lock
            self.camera_pitch =
                self.camera_pitch.clamp(-std::f32::consts::PI / 2.0 + 0.1, std::f32::consts::PI / 2.0 - 0.1);

            // Wrap yaw to [-PI, PI]
            if self.camera_yaw > std::f32::consts::PI {
                self.camera_yaw -= std::f32::consts::TAU;
            } else if self.camera_yaw < -std::f32::consts::PI {
                self.camera_yaw += std::f32::consts::TAU;
            }

            // Update camera basis vectors
            self.update_camera_basis();
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

            // Movement parameters
            self.acceleration_multiplier = rng.random_range(10.0..100.0);
            self.strafe_speed = rng.random_range(10.0..50.0);
            self.max_velocity = rng.random_range(100.0..500.0);
            self.damping = rng.random_range(0.9..0.99);

            // Visual parameters
            self.fov = rng.random_range(30.0_f32..90.0_f32).to_radians();
            self.star_size_base = rng.random_range(1.0..5.0);
            self.streak_velocity_threshold = rng.random_range(20.0..100.0);
            self.streak_length_multiplier = rng.random_range(0.1..2.0);

            // Star field parameters
            self.star_density = rng.random_range(0.0001..0.01);

            // Respawn stars with new density
            self.stars.clear();
            self.spawn_initial_stars();
        }

        pub fn toggle_panel(&mut self) {
            self.panel_visible = !self.panel_visible;
        }

        pub fn toggle_movement_mode(&mut self) {
            self.six_dof_mode = !self.six_dof_mode;
        }

        fn get_param_value(&self, index: usize) -> Option<f32> {
            match index {
                0 => Some(self.acceleration_multiplier),
                1 => Some(self.strafe_speed),
                2 => Some(self.max_velocity),
                3 => Some(self.damping),
                4 => Some(self.fov.to_degrees()),
                5 => Some(self.star_size_base),
                6 => Some(self.streak_velocity_threshold),
                7 => Some(self.streak_length_multiplier),
                8 => Some(self.star_density * 10000.0), // Scale for display
                _ => None,
            }
        }

        fn set_param_value(&mut self, index: usize, value: f32) {
            match index {
                0 => self.acceleration_multiplier = value.clamp(10.0, 200.0),
                1 => self.strafe_speed = value.clamp(10.0, 100.0),
                2 => self.max_velocity = value.clamp(50.0, 1000.0),
                3 => self.damping = value.clamp(0.8, 0.99),
                4 => self.fov = value.clamp(30.0, 120.0).to_radians(),
                5 => self.star_size_base = value.clamp(0.5, 10.0),
                6 => self.streak_velocity_threshold = value.clamp(10.0, 200.0),
                7 => self.streak_length_multiplier = value.clamp(0.1, 5.0),
                8 => {
                    self.star_density = (value / 10000.0).clamp(0.0001, 0.01);
                    self.stars.clear();
                    self.spawn_initial_stars();
                }
                _ => {}
            }
        }

        fn get_param_range(&self, index: usize) -> Option<(f32, f32)> {
            match index {
                0 => Some((10.0, 200.0)),
                1 => Some((10.0, 100.0)),
                2 => Some((50.0, 1000.0)),
                3 => Some((0.8, 0.99)),
                4 => Some((30.0, 120.0)),
                5 => Some((0.5, 10.0)),
                6 => Some((10.0, 200.0)),
                7 => Some((0.1, 5.0)),
                8 => Some((1.0, 100.0)),
                _ => None,
            }
        }

        pub fn update(&mut self, _delta_time: f64) {
            // Calculate actual delta time
            let current_time =
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64();
            let dt = (current_time - self.last_update_time).min(0.1) as f32; // Cap at 100ms
            self.last_update_time = current_time;

            // Apply acceleration to velocity
            let accel_x = self.camera_forward.0 * self.forward_accel * self.acceleration_multiplier;
            let accel_y = self.camera_forward.1 * self.forward_accel * self.acceleration_multiplier;
            let accel_z = self.camera_forward.2 * self.forward_accel * self.acceleration_multiplier;

            // Apply strafe movement based on mode
            let (strafe_x, strafe_y, strafe_z) = if self.six_dof_mode {
                // 6DOF space movement
                // Left stick X: strafe left/right
                // Left stick Y: strafe up/down (vertical movement)
                (
                    self.camera_right.0 * self.strafe_velocity.0 * self.strafe_speed
                        + self.camera_up.0 * self.strafe_velocity.1 * self.strafe_speed,
                    self.camera_right.1 * self.strafe_velocity.0 * self.strafe_speed
                        + self.camera_up.1 * self.strafe_velocity.1 * self.strafe_speed,
                    self.camera_right.2 * self.strafe_velocity.0 * self.strafe_speed
                        + self.camera_up.2 * self.strafe_velocity.1 * self.strafe_speed,
                )
            } else {
                // FPS-style movement
                // Left stick X: strafe left/right
                // Left stick Y: move forward/back
                (
                    self.camera_right.0 * self.strafe_velocity.0 * self.strafe_speed
                        + self.camera_forward.0 * self.strafe_velocity.1 * self.strafe_speed,
                    self.camera_right.1 * self.strafe_velocity.0 * self.strafe_speed
                        + self.camera_forward.1 * self.strafe_velocity.1 * self.strafe_speed,
                    self.camera_right.2 * self.strafe_velocity.0 * self.strafe_speed
                        + self.camera_forward.2 * self.strafe_velocity.1 * self.strafe_speed,
                )
            };

            // Update velocity
            self.camera_velocity.0 += (accel_x + strafe_x) * dt;
            self.camera_velocity.1 += (accel_y + strafe_y) * dt;
            self.camera_velocity.2 += (accel_z + strafe_z) * dt;

            // Apply damping
            self.camera_velocity.0 *= self.damping.powf(dt * 60.0);
            self.camera_velocity.1 *= self.damping.powf(dt * 60.0);
            self.camera_velocity.2 *= self.damping.powf(dt * 60.0);

            // Clamp to max velocity
            let vel_mag =
                (self.camera_velocity.0.powi(2) + self.camera_velocity.1.powi(2) + self.camera_velocity.2.powi(2))
                    .sqrt();
            if vel_mag > self.max_velocity {
                let scale = self.max_velocity / vel_mag;
                self.camera_velocity.0 *= scale;
                self.camera_velocity.1 *= scale;
                self.camera_velocity.2 *= scale;
            }

            // Update camera position
            self.camera_pos.0 += self.camera_velocity.0 * dt;
            self.camera_pos.1 += self.camera_velocity.1 * dt;
            self.camera_pos.2 += self.camera_velocity.2 * dt;

            // Update speed display
            if let Some(ref mut display) = self.speed_display {
                display.set_text(format!("Speed: {:.1}", vel_mag));
            }

            // Update parameter displays
            if self.param_displays.len() > 4 {
                self.param_displays[2].set_text(format!(
                    "Position: ({:.0}, {:.0}, {:.0})",
                    self.camera_pos.0, self.camera_pos.1, self.camera_pos.2
                ));
                self.param_displays[3].set_text(format!("Velocity: {:.1}", vel_mag));
                self.param_displays[4].set_text(format!("Yaw: {:.1}°", self.camera_yaw.to_degrees()));
                self.param_displays[5].set_text(format!("Pitch: {:.1}°", self.camera_pitch.to_degrees()));

                // Update movement mode display
                if self.param_displays.len() > 8 {
                    self.param_displays[8]
                        .set_text(format!("Mode: {}", if self.six_dof_mode { "6DOF Space" } else { "FPS Style" }));
                }

                if self.param_displays.len() > 21 {
                    self.param_displays[21].set_text(format!("Star Count: {}", self.stars.len()));
                }
            }

            // Spawn/despawn stars based on camera movement
            self.update_star_field();

            // Update code posters
            self.update_code_posters();
        }

        // Manage star spawning and despawning
        fn update_star_field(&mut self) {
            let mut to_remove = Vec::new();

            // Check for stars to despawn
            for (i, star) in self.stars.iter().enumerate() {
                let dx = star.pos.0 - self.camera_pos.0;
                let dy = star.pos.1 - self.camera_pos.1;
                let dz = star.pos.2 - self.camera_pos.2;
                let dist_sq = dx * dx + dy * dy + dz * dz;

                if dist_sq > self.despawn_radius * self.despawn_radius {
                    to_remove.push(i);
                }
            }

            // Remove despawned stars (in reverse order to maintain indices)
            for &i in to_remove.iter().rev() {
                self.stars.swap_remove(i);
            }

            // Calculate how many stars we should have
            let volume = (4.0 / 3.0) * std::f32::consts::PI * self.spawn_radius.powi(3);
            let target_count = (volume * self.star_density) as usize;

            // Spawn new stars if needed
            if self.stars.len() < target_count {
                let mut rng = rand::rng();
                let to_spawn = target_count - self.stars.len();

                for _ in 0..to_spawn {
                    // Random position in sphere around camera
                    let theta = rng.random_range(0.0..std::f32::consts::TAU);
                    let phi = rng.random_range(0.0..std::f32::consts::PI);
                    let r = rng.random_range(self.spawn_radius * 0.8..self.spawn_radius);

                    let x = self.camera_pos.0 + r * phi.sin() * theta.cos();
                    let y = self.camera_pos.1 + r * phi.sin() * theta.sin();
                    let z = self.camera_pos.2 + r * phi.cos();

                    let brightness = rng.random_range(100..255);
                    let size = rng.random_range(0.5..2.0);

                    self.stars.push(StarData { pos: (x, y, z), brightness, size });
                }
            }
        }

        // Load content for a poster if needed
        fn load_poster_content(&mut self, poster_idx: usize) {
            if poster_idx >= self.code_posters.len() {
                return;
            }

            let poster = &mut self.code_posters[poster_idx];
            if poster.content.is_none() {
                let start = std::time::Instant::now();
                match std::fs::read_to_string(&poster.file_path) {
                    Ok(content) => {
                        poster.content = Some(content);
                    }
                    Err(e) => {
                        eprintln!("WARNING: Failed to load content for {}: {}", poster.display_name, e);
                    }
                }
                let elapsed = start.elapsed();
                if elapsed.as_millis() > 16 {
                    eprintln!(
                        "WARNING: load_poster_content for '{}' took {}ms (>16ms frame budget)",
                        poster.display_name,
                        elapsed.as_millis()
                    );
                }
            }
        }

        // Update code posters based on camera position
        fn update_code_posters(&mut self) {
            let start = std::time::Instant::now();

            // First pass: determine which posters need content based on distance
            let mut needs_content = Vec::new();
            for (idx, poster) in self.code_posters.iter().enumerate() {
                let dx = poster.pos.0 - self.camera_pos.0;
                let dy = poster.pos.1 - self.camera_pos.1;
                let dz = poster.pos.2 - self.camera_pos.2;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                if dist < self.max_poster_distance && poster.content.is_none() {
                    needs_content.push(idx);
                }
            }

            // Load content for nearby posters
            if !needs_content.is_empty() {
                let load_start = std::time::Instant::now();
                let num_to_load = needs_content.len();
                for idx in needs_content {
                    self.load_poster_content(idx);
                }
                let load_elapsed = load_start.elapsed();
                if load_elapsed.as_millis() > 16 {
                    eprintln!(
                        "WARNING: loading {} poster files took {}ms (>16ms frame budget)",
                        num_to_load,
                        load_elapsed.as_millis()
                    );
                }
            }

            // Second pass: update lines_to_show for all posters
            for poster in self.code_posters.iter_mut() {
                let dx = poster.pos.0 - self.camera_pos.0;
                let dy = poster.pos.1 - self.camera_pos.1;
                let dz = poster.pos.2 - self.camera_pos.2;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                if dist < self.max_poster_distance {
                    let visibility = 1.0 - (dist / self.max_poster_distance);
                    poster.lines_to_show = (visibility * 50.0) as usize; // Max 50 lines
                } else {
                    poster.lines_to_show = 0;
                }
            }

            let elapsed = start.elapsed();
            if elapsed.as_millis() > 16 {
                eprintln!("WARNING: update_code_posters took {}ms (>16ms frame budget)", elapsed.as_millis());
            }

            // TODO: Add spawning/despawning logic similar to stars
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
            // Early exit if rect not set
            if self.rect.is_none() {
                return;
            }

            // Update simulation
            self.update(0.016);

            // Make sure atlases are registered
            if self.atlas_ids.is_empty() {
                self.setup_gpu_rendering(gpu_renderer);
            }

            if let Some(rect) = &self.rect {
                let (rx, ry, rw, rh) = rect.bounds();
                let screen_center_x = rx + rw / 2.0;
                let screen_center_y = ry + rh / 2.0;

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
                let mut _visible_count = 0;
                let vel_mag =
                    (self.camera_velocity.0.powi(2) + self.camera_velocity.1.powi(2) + self.camera_velocity.2.powi(2))
                        .sqrt();

                // Calculate FOV scale
                let fov_scale = (rh / 2.0) / (self.fov / 2.0).tan() as f64;

                // Sort stars by distance for proper rendering order (far to near)
                let mut star_render_data: Vec<(f32, f64, f64, StarData)> = Vec::new();

                for star in &self.stars {
                    // Transform star to view space
                    let dx = star.pos.0 - self.camera_pos.0;
                    let dy = star.pos.1 - self.camera_pos.1;
                    let dz = star.pos.2 - self.camera_pos.2;

                    // Apply view matrix (rotate by camera orientation)
                    let view_x = self.camera_right.0 * dx + self.camera_right.1 * dy + self.camera_right.2 * dz;
                    let view_y = self.camera_up.0 * dx + self.camera_up.1 * dy + self.camera_up.2 * dz;
                    let view_z =
                        -(self.camera_forward.0 * dx + self.camera_forward.1 * dy + self.camera_forward.2 * dz);

                    // Skip stars behind camera
                    if view_z <= 0.1 {
                        continue;
                    }

                    // Skip stars too far away
                    if view_z > self.max_render_distance {
                        continue;
                    }

                    // Project to screen space
                    let screen_x = screen_center_x + (view_x / view_z) as f64 * fov_scale;
                    let screen_y = screen_center_y + (view_y / view_z) as f64 * fov_scale;

                    // Check if on screen
                    if screen_x >= rx - 50.0
                        && screen_x <= rx + rw + 50.0
                        && screen_y >= ry - 50.0
                        && screen_y <= ry + rh + 50.0
                    {
                        _visible_count += 1;
                        star_render_data.push((view_z, screen_x, screen_y, *star));
                    }
                }

                // Sort by depth (far to near)
                star_render_data.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

                // Limit to reasonable number of stars to prevent GPU buffer overflow
                const MAX_VISIBLE_STARS: usize = 50000;
                if star_render_data.len() > MAX_VISIBLE_STARS {
                    star_render_data.truncate(MAX_VISIBLE_STARS);
                }

                // Render stars
                for (view_z, screen_x, screen_y, star) in star_render_data {
                    // Choose atlas based on size
                    let atlas_idx = if star.size > 1.5 {
                        2
                    } else if star.size > 1.0 {
                        1
                    } else {
                        0
                    };

                    if let Some(Some(atlas_id)) = self.atlas_ids.get(atlas_idx) {
                        // Calculate size based on distance
                        let size = (star.size * self.star_size_base / view_z.sqrt()) as f64;

                        // Calculate brightness based on distance
                        let distance_fade = (1.0 - view_z / self.max_render_distance).max(0.0);
                        let brightness =
                            (star.brightness as f32 * distance_fade * self.star_brightness_base / 255.0).min(1.0);

                        // Draw velocity streaks if moving fast
                        if vel_mag > self.streak_velocity_threshold {
                            // Calculate streak based on velocity direction in screen space
                            let velocity_screen_x = self.camera_velocity.0 * self.camera_right.0
                                + self.camera_velocity.1 * self.camera_right.1
                                + self.camera_velocity.2 * self.camera_right.2;
                            let velocity_screen_y = self.camera_velocity.0 * self.camera_up.0
                                + self.camera_velocity.1 * self.camera_up.1
                                + self.camera_velocity.2 * self.camera_up.2;

                            let streak_length =
                                (vel_mag - self.streak_velocity_threshold) * self.streak_length_multiplier / view_z;
                            let vel_norm = ((velocity_screen_x * velocity_screen_x
                                + velocity_screen_y * velocity_screen_y)
                                .sqrt())
                            .max(0.001);
                            let streak_dx = (velocity_screen_x / vel_norm * streak_length) as f32;
                            let streak_dy = (velocity_screen_y / vel_norm * streak_length) as f32;

                            // Draw streak line
                            gpu_renderer.add_line(
                                screen_x as f32,
                                screen_y as f32,
                                screen_x as f32 - streak_dx,
                                screen_y as f32 - streak_dy,
                                1.0,
                                [brightness, brightness, brightness, brightness * 0.5],
                            );
                        }

                        // Draw star dot
                        gpu_renderer.add_textured_rect(
                            (screen_x - size / 2.0) as f32,
                            (screen_y - size / 2.0) as f32,
                            size as f32,
                            size as f32,
                            *atlas_id,
                            [brightness, brightness, brightness, 1.0],
                        );
                    }
                }

                // Draw code posters
                self.render_code_posters(gpu_renderer, rx, ry, rw, rh, screen_center_x, screen_center_y, fov_scale);

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
                        (None, 0),     // Title
                        (None, 1),     // Camera header
                        (None, 2),     // Position display
                        (None, 3),     // Velocity display
                        (None, 4),     // Yaw display
                        (None, 5),     // Pitch display
                        (None, 6),     // blank
                        (None, 7),     // Movement header
                        (None, 8),     // Mode display
                        (Some(0), 9),  // Acceleration
                        (Some(1), 10), // Strafe Speed
                        (Some(2), 11), // Max Velocity
                        (Some(3), 12), // Damping
                        (None, 13),    // blank
                        (None, 14),    // Visual header
                        (Some(4), 15), // FOV
                        (Some(5), 16), // Star Size
                        (None, 17),    // Render Distance display
                        (Some(6), 18), // Streak Threshold
                        (Some(7), 19), // Streak Length (multiplier)
                        (None, 20),    // blank
                        (None, 21),    // Star Field header
                        (None, 22),    // Star Count display
                        (Some(8), 23), // Density
                        (None, 24),    // Spawn Radius display
                    ];

                    // First, collect all the data we need
                    let mut display_updates = Vec::new();
                    let param_names = [
                        "Acceleration",     // 0
                        "Strafe Speed",     // 1
                        "Max Velocity",     // 2
                        "Damping",          // 3
                        "FOV",              // 4
                        "Star Size",        // 5
                        "Streak Threshold", // 6
                        "Streak Length",    // 7
                        "Star Density",     // 8
                    ];

                    for (param_idx, display_idx) in param_indices.iter() {
                        let text_and_color = if let Some(idx) = param_idx {
                            if let Some(value) = self.get_param_value(*idx) {
                                let name = param_names.get(*idx).unwrap_or(&"Unknown");

                                // Special formatting for parameters
                                let value_str = match idx {
                                    2 => format!("{:.0}", value),  // max velocity
                                    3 => format!("{:.3}", value),  // damping (needs precision)
                                    4 => format!("{:.0}°", value), // FOV in degrees
                                    8 => format!("{:.4}", value),  // density (small number)
                                    _ => format!("{:.1}", value),
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

        fn render_code_posters(
            &mut self,
            gpu_renderer: &mut dyn ::hotline::GpuRenderingContext,
            rx: f64,
            ry: f64,
            rw: f64,
            rh: f64,
            screen_center_x: f64,
            screen_center_y: f64,
            fov_scale: f64,
        ) {
            let start = std::time::Instant::now();

            // Ensure registry is available for creating text renderers
            let registry_start = std::time::Instant::now();
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }
            let registry_elapsed = registry_start.elapsed();
            if registry_elapsed.as_micros() > 500 {
                eprintln!(
                    "WARNING: render_code_posters registry setup took {}μs (>500μs budget)",
                    registry_elapsed.as_micros()
                );
            }

            // Sort posters by distance (far to near) for proper rendering
            let sorting_start = std::time::Instant::now();
            let mut poster_render_data: Vec<(usize, f32, f64, f64)> = Vec::new();

            for (idx, poster) in self.code_posters.iter().enumerate() {
                if poster.lines_to_show == 0 || poster.content.is_none() {
                    continue;
                }

                // Transform poster position to view space
                let dx = poster.pos.0 - self.camera_pos.0;
                let dy = poster.pos.1 - self.camera_pos.1;
                let dz = poster.pos.2 - self.camera_pos.2;

                // Apply view matrix
                let view_x = self.camera_right.0 * dx + self.camera_right.1 * dy + self.camera_right.2 * dz;
                let view_y = self.camera_up.0 * dx + self.camera_up.1 * dy + self.camera_up.2 * dz;
                let view_z = -(self.camera_forward.0 * dx + self.camera_forward.1 * dy + self.camera_forward.2 * dz);

                // Skip posters behind camera
                if view_z <= 0.1 {
                    continue;
                }

                // Project to screen space
                let screen_x = screen_center_x + (view_x / view_z) as f64 * fov_scale;
                let screen_y = screen_center_y + (view_y / view_z) as f64 * fov_scale;

                // Check if poster would be on screen (with some margin)
                let margin = 100.0;
                if screen_x >= rx - margin
                    && screen_x <= rx + rw + margin
                    && screen_y >= ry - margin
                    && screen_y <= ry + rh + margin
                {
                    poster_render_data.push((idx, view_z, screen_x, screen_y));
                }
            }

            // Sort by depth (far to near)
            poster_render_data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            let sorting_elapsed = sorting_start.elapsed();
            if sorting_elapsed.as_micros() > 500 {
                eprintln!(
                    "WARNING: render_code_posters sorting/culling took {}μs (>500μs budget) for {} posters",
                    sorting_elapsed.as_micros(),
                    poster_render_data.len()
                );
            }

            // Conditional debug: warn if we have posters but none are visible
            if self.code_posters.len() > 0 && poster_render_data.is_empty() {
                // Count how many have content and lines to show
                let with_content = self.code_posters.iter().filter(|p| p.content.is_some()).count();
                let with_lines = self.code_posters.iter().filter(|p| p.lines_to_show > 0).count();
                eprintln!(
                    "WARNING: No posters visible! total={}, with_content={}, with_lines={}",
                    self.code_posters.len(),
                    with_content,
                    with_lines
                );
            }

            // Render each visible poster
            for (idx, view_z, screen_x, screen_y) in poster_render_data {
                let poster_start = std::time::Instant::now();
                let poster = &self.code_posters[idx];

                // Calculate poster size based on distance
                let scale = fov_scale / view_z as f64;
                let poster_width = (poster.width as f64 * scale) as f32;
                let poster_height = (poster.height as f64 * scale) as f32;

                // Draw poster background
                let bg_color = [
                    poster.color.0 as f32 / 255.0 * 0.3,
                    poster.color.1 as f32 / 255.0 * 0.3,
                    poster.color.2 as f32 / 255.0 * 0.3,
                    0.95,
                ];

                gpu_renderer.add_solid_rect(
                    (screen_x - poster_width as f64 / 2.0) as f32,
                    (screen_y - poster_height as f64 / 2.0) as f32,
                    poster_width,
                    poster_height,
                    bg_color,
                );

                // Draw poster border
                let border_color = [
                    poster.color.0 as f32 / 255.0,
                    poster.color.1 as f32 / 255.0,
                    poster.color.2 as f32 / 255.0,
                    poster.color.3 as f32 / 255.0,
                ];

                let border_thickness = (2.0 * scale as f32 / 10.0).max(1.0);
                let bx = (screen_x - poster_width as f64 / 2.0) as f32;
                let by = (screen_y - poster_height as f64 / 2.0) as f32;

                // Top border
                gpu_renderer.add_solid_rect(bx, by, poster_width, border_thickness, border_color);
                // Bottom border
                gpu_renderer.add_solid_rect(
                    bx,
                    by + poster_height - border_thickness,
                    poster_width,
                    border_thickness,
                    border_color,
                );
                // Left border
                gpu_renderer.add_solid_rect(bx, by, border_thickness, poster_height, border_color);
                // Right border
                gpu_renderer.add_solid_rect(
                    bx + poster_width - border_thickness,
                    by,
                    border_thickness,
                    poster_height,
                    border_color,
                );

                // Create or update text renderers for this poster
                if !self.poster_text_renderers.contains_key(&idx) {
                    self.poster_text_renderers.insert(idx, Vec::new());
                }

                let text_renderers = self.poster_text_renderers.get_mut(&idx).unwrap();

                // Render file path as title
                let _title_scale = (scale as f32 / 20.0).clamp(0.5, 2.0);
                let title_y = screen_y - poster_height as f64 / 2.0 + 5.0;

                if text_renderers.is_empty() {
                    let new_start = std::time::Instant::now();
                    let mut title_renderer = TextRenderer::new();
                    title_renderer.set_text(poster.display_name.clone());
                    title_renderer.set_color(poster.color);
                    text_renderers.push(title_renderer);
                    let new_elapsed = new_start.elapsed();
                    if new_elapsed.as_micros() > 500 {
                        eprintln!(
                            "WARNING: TextRenderer::new for title '{}' took {}μs (>500μs budget)",
                            poster.display_name,
                            new_elapsed.as_micros()
                        );
                    }
                }

                let title_render_start = std::time::Instant::now();
                text_renderers[0].set_x(screen_x - poster_width as f64 / 2.0 + 5.0);
                text_renderers[0].set_y(title_y);
                text_renderers[0].render_gpu(gpu_renderer);
                let title_render_elapsed = title_render_start.elapsed();
                if title_render_elapsed.as_micros() > 500 {
                    eprintln!("WARNING: title render_gpu took {}μs (>500μs budget)", title_render_elapsed.as_micros());
                }

                // Render code lines
                if let Some(content) = &poster.content {
                    let lines: Vec<&str> = content.lines().take(poster.lines_to_show).collect();
                    let line_height = 14.0 * scale as f64 / 20.0;
                    let start_y = title_y + 20.0 * scale as f64 / 20.0;

                    // Ensure we have enough text renderers
                    if text_renderers.len() <= lines.len() {
                        let create_start = std::time::Instant::now();
                        let needed = lines.len() + 1 - text_renderers.len();
                        while text_renderers.len() <= lines.len() {
                            let mut line_renderer = TextRenderer::new();
                            line_renderer.set_color((200, 200, 200, 255));
                            text_renderers.push(line_renderer);
                        }
                        let create_elapsed = create_start.elapsed();
                        if create_elapsed.as_millis() > 2 {
                            eprintln!(
                                "WARNING: creating {} TextRenderers took {}ms (>2ms budget)",
                                needed,
                                create_elapsed.as_millis()
                            );
                        }
                    }

                    let lines_render_start = std::time::Instant::now();
                    for (i, line) in lines.iter().enumerate() {
                        let line_start = std::time::Instant::now();
                        let line_y = start_y + i as f64 * line_height;
                        let renderer_idx = i + 1; // +1 because title is at index 0

                        let set_text_start = std::time::Instant::now();
                        text_renderers[renderer_idx].set_text(line.to_string());
                        let set_text_elapsed = set_text_start.elapsed();
                        if set_text_elapsed.as_micros() > 100 {
                            eprintln!(
                                "WARNING: set_text for line {} took {}μs (>100μs budget)",
                                i,
                                set_text_elapsed.as_micros()
                            );
                        }

                        text_renderers[renderer_idx].set_x(screen_x - poster_width as f64 / 2.0 + 10.0);
                        text_renderers[renderer_idx].set_y(line_y);

                        let render_start = std::time::Instant::now();
                        text_renderers[renderer_idx].render_gpu(gpu_renderer);
                        let render_elapsed = render_start.elapsed();
                        if render_elapsed.as_micros() > 100 {
                            eprintln!(
                                "WARNING: render_gpu for line {} took {}μs (>100μs budget)",
                                i,
                                render_elapsed.as_micros()
                            );
                        }

                        let line_elapsed = line_start.elapsed();
                        if line_elapsed.as_micros() > 200 {
                            eprintln!(
                                "WARNING: rendering line {} took {}μs total (>200μs budget)",
                                i,
                                line_elapsed.as_micros()
                            );
                        }
                    }
                    let lines_render_elapsed = lines_render_start.elapsed();
                    if lines_render_elapsed.as_millis() > 2 {
                        eprintln!(
                            "WARNING: rendering {} lines took {}ms (>2ms budget)",
                            lines.len(),
                            lines_render_elapsed.as_millis()
                        );
                    }
                }

                let poster_elapsed = poster_start.elapsed();
                if poster_elapsed.as_millis() > 2 {
                    eprintln!(
                        "WARNING: rendering poster '{}' took {}ms (>2ms budget)",
                        poster.display_name,
                        poster_elapsed.as_millis()
                    );
                }
            }

            let elapsed = start.elapsed();
            if elapsed.as_millis() > 16 {
                eprintln!("WARNING: render_code_posters took {}ms (>16ms frame budget)", elapsed.as_millis());
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
                        None,    // 0: Title
                        None,    // 1: Camera header
                        None,    // 2: Position display
                        None,    // 3: Velocity display
                        None,    // 4: Yaw display
                        None,    // 5: Pitch display
                        None,    // 6: blank
                        None,    // 7: Movement header
                        None,    // 8: Mode display
                        Some(0), // 9: Acceleration
                        Some(1), // 10: Strafe Speed
                        Some(2), // 11: Max Velocity
                        Some(3), // 12: Damping
                        None,    // 13: blank
                        None,    // 14: Visual header
                        Some(4), // 15: FOV
                        Some(5), // 16: Star Size
                        None,    // 17: Render Distance display
                        Some(6), // 18: Streak Threshold
                        Some(7), // 19: Streak Length
                        None,    // 20: blank
                        None,    // 21: Star Field header
                        None,    // 22: Star Count display
                        Some(8), // 23: Density
                        None,    // 24: Spawn Radius display
                    ];

                    if param_index < param_map.len() {
                        if let Some(idx) = param_map.get(param_index).and_then(|&p| p) {
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
                        None,    // 0: Title
                        None,    // 1: Camera header
                        None,    // 2: Position display
                        None,    // 3: Velocity display
                        None,    // 4: Yaw display
                        None,    // 5: Pitch display
                        None,    // 6: blank
                        None,    // 7: Movement header
                        None,    // 8: Mode display
                        Some(0), // 9: Acceleration
                        Some(1), // 10: Strafe Speed
                        Some(2), // 11: Max Velocity
                        Some(3), // 12: Damping
                        None,    // 13: blank
                        None,    // 14: Visual header
                        Some(4), // 15: FOV
                        Some(5), // 16: Star Size
                        None,    // 17: Render Distance display
                        Some(6), // 18: Streak Threshold
                        Some(7), // 19: Streak Length
                        None,    // 20: blank
                        None,    // 21: Star Field header
                        None,    // 22: Star Count display
                        Some(8), // 23: Density
                        None,    // 24: Spawn Radius display
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
