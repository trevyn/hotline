hotline::object!({
    #[derive(Default)]
    pub struct GameController {
        rect: Option<Rect>,
        connected: bool,
        // Axes: left stick x/y, right stick x/y, left trigger, right trigger
        axes: [f32; 6],
        // Buttons: A, B, X, Y, Back, Guide, Start, LeftStick, RightStick,
        //          LeftShoulder, RightShoulder, DPad Up/Down/Left/Right
        buttons: [bool; 15],
        labels: Vec<TextRenderer>,
        axis_labels: Vec<TextRenderer>,
        #[serde(skip)]
        controller_id: Option<u32>,
        background_atlas_id: Option<u32>,
        border_atlas_id: Option<u32>,
        bar_bg_atlas_id: Option<u32>,
        bar_fg_atlas_id: Option<u32>,
        button_off_atlas_id: Option<u32>,
        button_on_atlas_id: Option<u32>,
        circle_atlas_id: Option<u32>,
        filled_circle_atlas_id: Option<u32>,
    }

    impl GameController {
        pub fn initialize(&mut self) {
            // Create labels for display
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            // Title label
            let title = TextRenderer::new().with_text("Game Controller".to_string()).with_color((255, 255, 255, 255));
            self.labels.push(title);

            // Connection status
            let status = TextRenderer::new().with_text("Disconnected".to_string()).with_color((128, 128, 128, 255));
            self.labels.push(status);

            // Axis labels
            let axis_names = ["LX", "LY", "RX", "RY", "LT", "RT"];
            for name in &axis_names {
                let label = TextRenderer::new().with_text(format!("{}: 0.00", name)).with_color((200, 200, 200, 255));
                self.axis_labels.push(label);
            }
        }

        pub fn set_rect(&mut self, rect: Rect) {
            let (x, y, w, h) = rect.bounds();
            hotline::debug_rate_limited!(
                "gc_set_rect",
                1000,
                "GameController set_rect called with bounds: x={}, y={}, w={}, h={}",
                x,
                y,
                w,
                h
            );
            self.rect = Some(rect);
        }

        pub fn set_connected(&mut self, connected: bool, id: Option<u32>) {
            self.connected = connected;
            self.controller_id = id;
            if let Some(status_label) = self.labels.get_mut(1) {
                if connected {
                    status_label.set_text(format!("Connected (ID: {})", id.unwrap_or(0)));
                    status_label.set_color((0, 255, 0, 255));
                } else {
                    status_label.set_text("Disconnected".to_string());
                    status_label.set_color((128, 128, 128, 255));
                }
            }
        }

        pub fn update_axis(&mut self, axis: u8, value: f32) {
            if (axis as usize) < self.axes.len() {
                self.axes[axis as usize] = value;

                // Update label
                if let Some(label) = self.axis_labels.get_mut(axis as usize) {
                    let names = ["LX", "LY", "RX", "RY", "LT", "RT"];
                    label.set_text(format!("{}: {:.2}", names[axis as usize], value));
                }
            }
        }

        pub fn axis_values(&self) -> (f32, f32, f32, f32) {
            (self.axes[0], self.axes[1], self.axes[2], self.axes[3])
        }

        pub fn update_button(&mut self, button: u8, pressed: bool) {
            if (button as usize) < self.buttons.len() {
                self.buttons[button as usize] = pressed;
            }
        }

        pub fn register_atlases(&mut self, gpu_renderer: &mut GPURenderer) {
            hotline::debug_rate_limited!("gc_register_atlases", 1000, "GameController register_atlases called");

            // Background (dark gray)
            if self.background_atlas_id.is_none() {
                let bg_pixel = vec![50u8, 50, 50, 255]; // RGBA
                let id = gpu_renderer.register_atlas(bg_pixel, 1, 1, AtlasFormat::RGBA);
                self.background_atlas_id = Some(id);
                hotline::debug_rate_limited!("gc_register_bg", 1000, "Registered background_atlas_id: {}", id);
            }

            // Border (gray)
            if self.border_atlas_id.is_none() {
                let border_pixel = vec![128u8, 128, 128, 255];
                let id = gpu_renderer.register_atlas(border_pixel, 1, 1, AtlasFormat::RGBA);
                self.border_atlas_id = Some(id);
            }

            // Bar background (dark gray)
            if self.bar_bg_atlas_id.is_none() {
                let bar_bg_pixel = vec![64u8, 64, 64, 255];
                let id = gpu_renderer.register_atlas(bar_bg_pixel, 1, 1, AtlasFormat::RGBA);
                self.bar_bg_atlas_id = Some(id);
            }

            // Bar foreground (pink)
            if self.bar_fg_atlas_id.is_none() {
                let bar_fg_pixel = vec![255u8, 105, 180, 255]; // RGBA format: Red=255, Green=105, Blue=180, Alpha=255
                let id = gpu_renderer.register_atlas(bar_fg_pixel, 1, 1, AtlasFormat::RGBA);
                self.bar_fg_atlas_id = Some(id);
                hotline::debug_rate_limited!(
                    "gc_register_bar_fg",
                    1000,
                    "Registered bar_fg_atlas_id: {} (pink pixel)",
                    id
                );
            }

            // Button off (gray)
            if self.button_off_atlas_id.is_none() {
                let button_off_pixel = vec![100u8, 100, 100, 255];
                let id = gpu_renderer.register_atlas(button_off_pixel, 1, 1, AtlasFormat::RGBA);
                self.button_off_atlas_id = Some(id);
            }

            // Button on (pink)
            if self.button_on_atlas_id.is_none() {
                let button_on_pixel = vec![255u8, 105, 180, 255];
                let id = gpu_renderer.register_atlas(button_on_pixel, 1, 1, AtlasFormat::RGBA);
                self.button_on_atlas_id = Some(id);
            }

            // Circle outline texture
            if self.circle_atlas_id.is_none() {
                let size = 64;
                let radius = (size / 2) as f32 - 1.0;
                let center = (size / 2) as f32;
                let mut circle_data = vec![0u8; size * size * 4];

                for y in 0..size {
                    for x in 0..size {
                        let dx = x as f32 - center;
                        let dy = y as f32 - center;
                        let dist = (dx * dx + dy * dy).sqrt();

                        let idx = (y * size + x) * 4;
                        if (dist - radius).abs() < 2.0 {
                            // White outline
                            circle_data[idx] = 255;
                            circle_data[idx + 1] = 255;
                            circle_data[idx + 2] = 255;
                            circle_data[idx + 3] = 255;
                        }
                    }
                }

                let id = gpu_renderer.register_atlas(circle_data, size as u32, size as u32, AtlasFormat::RGBA);
                self.circle_atlas_id = Some(id);
                hotline::debug_rate_limited!("gc_register_circle", 1000, "Created circle texture with id: {}", id);
            }

            // Filled circle texture
            if self.filled_circle_atlas_id.is_none() {
                let size = 16;
                let radius = (size / 2) as f32 - 0.5;
                let center = (size / 2) as f32;
                let mut circle_data = vec![0u8; size * size * 4];

                for y in 0..size {
                    for x in 0..size {
                        let dx = x as f32 - center;
                        let dy = y as f32 - center;
                        let dist = (dx * dx + dy * dy).sqrt();

                        let idx = (y * size + x) * 4;
                        if dist <= radius {
                            // Pink filled circle - ABGR format
                            circle_data[idx] = 255; // A
                            circle_data[idx + 1] = 180; // B
                            circle_data[idx + 2] = 105; // G
                            circle_data[idx + 3] = 255; // R
                        }
                    }
                }

                let id = gpu_renderer.register_atlas(circle_data, size as u32, size as u32, AtlasFormat::RGBA);
                self.filled_circle_atlas_id = Some(id);
                hotline::debug_rate_limited!(
                    "gc_register_filled_circle",
                    1000,
                    "Created filled circle texture with id: {}",
                    id
                );
            }

            // Register text renderer atlases
            for label in &mut self.labels {
                label.register_atlas(gpu_renderer);
            }
            for label in &mut self.axis_labels {
                label.register_atlas(gpu_renderer);
            }
        }

        pub fn generate_commands(&mut self, gpu_renderer: &mut GPURenderer) {
            // Re-register atlases if needed (for hot reload)
            if self.circle_atlas_id.is_none() || self.filled_circle_atlas_id.is_none() {
                hotline::debug_rate_limited!("gc_reregister", 1000, "Re-registering atlases");
                self.register_atlases(gpu_renderer);
            }

            if let Some(rect) = &self.rect {
                let (x, y, w, h) = rect.clone().bounds();

                // Draw background
                if let Some(bg_id) = self.background_atlas_id {
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: bg_id,
                        dest_x: x,
                        dest_y: y,
                        dest_width: w,
                        dest_height: h,
                        rotation: 0.0,
                        color: (255, 255, 255, 255),
                    });
                }

                // Draw border (4 rectangles)
                if let Some(border_id) = self.border_atlas_id {
                    // Top
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: x,
                        dest_y: y,
                        dest_width: w,
                        dest_height: 1.0,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                    // Bottom
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: x,
                        dest_y: y + h - 1.0,
                        dest_width: w,
                        dest_height: 1.0,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                    // Left
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: x,
                        dest_y: y,
                        dest_width: 1.0,
                        dest_height: h,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                    // Right
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: x + w - 1.0,
                        dest_y: y,
                        dest_width: 1.0,
                        dest_height: h,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                }

                // Render text labels
                let mut label_y = y + 10.0;
                for label in &mut self.labels {
                    label.set_x(x + 10.0);
                    label.set_y(label_y);
                    label.generate_commands(gpu_renderer);
                    label_y += 20.0;
                }

                // Render axis visualizations
                label_y = y + 60.0;
                for (i, label) in self.axis_labels.iter_mut().enumerate() {
                    label.set_x(x + 10.0);
                    label.set_y(label_y + i as f64 * 20.0);
                    label.generate_commands(gpu_renderer);

                    // Draw axis bar visualization
                    let vis_x = x + 100.0;
                    let vis_y = label_y + i as f64 * 20.0 - 5.0;
                    let vis_w = 100.0;
                    let vis_h = 10.0;

                    // Background bar
                    if let Some(bar_bg_id) = self.bar_bg_atlas_id {
                        gpu_renderer.add_command(RenderCommand::Rect {
                            texture_id: bar_bg_id,
                            dest_x: vis_x,
                            dest_y: vis_y,
                            dest_width: vis_w,
                            dest_height: vis_h,
                            rotation: 0.0,
                            color: (255, 255, 255, 255), // White - no color modulation
                        });
                    }

                    // Value bar
                    if let Some(bar_fg_id) = self.bar_fg_atlas_id {
                        let value = self.axes[i];
                        if i < 4 {
                            // Sticks: -1 to 1, draw from center
                            let center = vis_x + vis_w / 2.0;
                            if value >= 0.0 {
                                gpu_renderer.add_command(RenderCommand::Rect {
                                    texture_id: bar_fg_id,
                                    dest_x: center,
                                    dest_y: vis_y,
                                    dest_width: (value as f64) * vis_w / 2.0,
                                    dest_height: vis_h,
                                    rotation: 0.0,
                                    color: (255, 255, 255, 255), // White - no color modulation
                                });
                            } else {
                                gpu_renderer.add_command(RenderCommand::Rect {
                                    texture_id: bar_fg_id,
                                    dest_x: center + (value as f64) * vis_w / 2.0,
                                    dest_y: vis_y,
                                    dest_width: -(value as f64) * vis_w / 2.0,
                                    dest_height: vis_h,
                                    rotation: 0.0,
                                    color: (255, 255, 255, 255), // White - no color modulation
                                });
                            }
                        } else {
                            // Triggers: 0 to 1, draw from left
                            gpu_renderer.add_command(RenderCommand::Rect {
                                texture_id: bar_fg_id,
                                dest_x: vis_x,
                                dest_y: vis_y,
                                dest_width: (value as f64) * vis_w,
                                dest_height: vis_h,
                                rotation: 0.0,
                                color: (255, 255, 255, 255), // White - no color modulation
                            });
                        }
                    }
                }

                // Draw button states
                let button_y = label_y + self.axis_labels.len() as f64 * 20.0 + 10.0;
                let mut button_x = x + 10.0;
                let mut button_row_y = button_y;

                let button_names =
                    ["A", "B", "X", "Y", "Back", "Guide", "Start", "LS", "RS", "LB", "RB", "D↑", "D↓", "D←", "D→"];

                for (i, _name) in button_names.iter().enumerate() {
                    if i < self.buttons.len() {
                        let is_pressed = self.buttons[i];
                        let atlas_id = if is_pressed { self.button_on_atlas_id } else { self.button_off_atlas_id };

                        // Draw button indicator (small square)
                        if let Some(btn_id) = atlas_id {
                            gpu_renderer.add_command(RenderCommand::Rect {
                                texture_id: btn_id,
                                dest_x: button_x,
                                dest_y: button_row_y,
                                dest_width: 10.0,
                                dest_height: 10.0,
                                rotation: 0.0,
                                color: (255, 255, 255, 255), // White - no color modulation
                            });
                        }

                        button_x += 30.0;
                        if (i + 1) % 5 == 0 {
                            button_x = x + 10.0;
                            button_row_y += 25.0;
                        }
                    }
                }

                // Draw analog stick visualizations
                // Place them in the middle of the controller area
                let stick_radius = 40.0;
                let stick_dot_radius = 8.0; // Match half of texture size (16/2)

                // Left stick - put it in visible area
                let left_stick_x = x + 60.0;
                let left_stick_y = y + 200.0; // Middle of the controller area

                // Drawing sticks

                // Draw left stick circle outline
                if let Some(circle_id) = self.circle_atlas_id {
                    // Drawing left stick circle
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: circle_id,
                        dest_x: left_stick_x - stick_radius,
                        dest_y: left_stick_y - stick_radius,
                        dest_width: stick_radius * 2.0,
                        dest_height: stick_radius * 2.0,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                }

                // Draw left stick position
                let lx = self.axes[0] as f64; // -1 to 1
                let ly = self.axes[1] as f64; // -1 to 1
                let left_dot_x = left_stick_x + lx * (stick_radius - stick_dot_radius);
                let left_dot_y = left_stick_y + ly * (stick_radius - stick_dot_radius);

                if let Some(filled_circle_id) = self.filled_circle_atlas_id {
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: filled_circle_id,
                        dest_x: left_dot_x - stick_dot_radius,
                        dest_y: left_dot_y - stick_dot_radius,
                        dest_width: stick_dot_radius * 2.0,
                        dest_height: stick_dot_radius * 2.0,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                }

                // Right stick
                let right_stick_x = x + 140.0;
                let right_stick_y = y + 200.0; // Same height as left stick

                // Draw right stick circle outline
                if let Some(circle_id) = self.circle_atlas_id {
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: circle_id,
                        dest_x: right_stick_x - stick_radius,
                        dest_y: right_stick_y - stick_radius,
                        dest_width: stick_radius * 2.0,
                        dest_height: stick_radius * 2.0,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                }

                // Draw right stick position
                let rx = self.axes[2] as f64; // -1 to 1
                let ry = self.axes[3] as f64; // -1 to 1
                let right_dot_x = right_stick_x + rx * (stick_radius - stick_dot_radius);
                let right_dot_y = right_stick_y + ry * (stick_radius - stick_dot_radius);

                if let Some(filled_circle_id) = self.filled_circle_atlas_id {
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: filled_circle_id,
                        dest_x: right_dot_x - stick_dot_radius,
                        dest_y: right_dot_y - stick_dot_radius,
                        dest_width: stick_dot_radius * 2.0,
                        dest_height: stick_dot_radius * 2.0,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                }

                // Draw controllable rectangle (controlled by left stick)
                let rect_area_y = y + 280.0; // Below the analog sticks
                let rect_area_x = x + 10.0;
                let rect_area_w = w - 20.0;
                let rect_area_h = 80.0;

                // Draw play area border
                if let Some(border_id) = self.border_atlas_id {
                    // Top
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: rect_area_x,
                        dest_y: rect_area_y,
                        dest_width: rect_area_w,
                        dest_height: 1.0,
                        rotation: 0.0,
                        color: (255, 128, 128, 128), // ABGR: Alpha=255, Blue=128, Green=128, Red=128 (gray)
                    });
                    // Bottom
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: rect_area_x,
                        dest_y: rect_area_y + rect_area_h,
                        dest_width: rect_area_w,
                        dest_height: 1.0,
                        rotation: 0.0,
                        color: (255, 128, 128, 128), // ABGR: Alpha=255, Blue=128, Green=128, Red=128 (gray)
                    });
                    // Left
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: rect_area_x,
                        dest_y: rect_area_y,
                        dest_width: 1.0,
                        dest_height: rect_area_h,
                        rotation: 0.0,
                        color: (255, 128, 128, 128), // ABGR: Alpha=255, Blue=128, Green=128, Red=128 (gray)
                    });
                    // Right
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: rect_area_x + rect_area_w,
                        dest_y: rect_area_y,
                        dest_width: 1.0,
                        dest_height: rect_area_h,
                        rotation: 0.0,
                        color: (255, 128, 128, 128), // ABGR: Alpha=255, Blue=128, Green=128, Red=128 (gray)
                    });
                }

                // Draw movable rectangle
                let rect_size = 20.0;
                // Map -1 to 1 range to play area bounds
                let rect_x = rect_area_x + (rect_area_w - rect_size) * (lx + 1.0) / 2.0;
                let rect_y = rect_area_y + (rect_area_h - rect_size) * (ly + 1.0) / 2.0;

                if let Some(bar_fg_id) = self.bar_fg_atlas_id {
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: bar_fg_id,
                        dest_x: rect_x,
                        dest_y: rect_y,
                        dest_width: rect_size,
                        dest_height: rect_size,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                }

                // Draw trigger visualization bars at bottom
                let trigger_y = y + h - 40.0; // Near bottom of controller area

                // Left trigger
                let lt_value = self.axes[4] as f64;
                let trigger_w = 80.0;
                let trigger_h = 15.0;

                if let Some(bar_bg_id) = self.bar_bg_atlas_id {
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: bar_bg_id,
                        dest_x: x + 20.0,
                        dest_y: trigger_y,
                        dest_width: trigger_w,
                        dest_height: trigger_h,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                }

                if let Some(bar_fg_id) = self.bar_fg_atlas_id {
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: bar_fg_id,
                        dest_x: x + 20.0,
                        dest_y: trigger_y,
                        dest_width: lt_value * trigger_w,
                        dest_height: trigger_h,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                }

                // Right trigger
                let rt_value = self.axes[5] as f64;

                if let Some(bar_bg_id) = self.bar_bg_atlas_id {
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: bar_bg_id,
                        dest_x: x + w - 100.0,
                        dest_y: trigger_y,
                        dest_width: trigger_w,
                        dest_height: trigger_h,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                }

                if let Some(bar_fg_id) = self.bar_fg_atlas_id {
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: bar_fg_id,
                        dest_x: x + w - 100.0,
                        dest_y: trigger_y,
                        dest_width: rt_value * trigger_w,
                        dest_height: trigger_h,
                        rotation: 0.0,
                        color: (255, 255, 255, 255), // White - no color modulation
                    });
                }
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            // GPU only - no CPU rendering
            let _ = (buffer, buffer_width, buffer_height, pitch);
        }
    }
});
// reload trigger v7
