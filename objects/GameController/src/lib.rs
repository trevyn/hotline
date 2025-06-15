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

        pub fn update_button(&mut self, button: u8, pressed: bool) {
            if (button as usize) < self.buttons.len() {
                self.buttons[button as usize] = pressed;
            }
        }

        fn draw_rect(
            &self,
            buffer: &mut [u8],
            buffer_width: i64,
            buffer_height: i64,
            pitch: i64,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            color: (u8, u8, u8, u8),
        ) {
            let x_start = x.max(0);
            let y_start = y.max(0);
            let x_end = (x + w).min(buffer_width as i32);
            let y_end = (y + h).min(buffer_height as i32);

            for py in y_start..y_end {
                for px in x_start..x_end {
                    let offset = (py as i64 * pitch + px as i64 * 4) as usize;
                    if offset + 3 < buffer.len() {
                        buffer[offset] = color.0; // B
                        buffer[offset + 1] = color.1; // G
                        buffer[offset + 2] = color.2; // R
                        buffer[offset + 3] = color.3; // A
                    }
                }
            }
        }

        fn draw_circle(
            &self,
            buffer: &mut [u8],
            buffer_width: i64,
            buffer_height: i64,
            pitch: i64,
            cx: f64,
            cy: f64,
            radius: f64,
            color: (u8, u8, u8, u8),
        ) {
            let r2 = radius * radius;
            let x_start = (cx - radius).max(0.0) as i32;
            let y_start = (cy - radius).max(0.0) as i32;
            let x_end = (cx + radius).min(buffer_width as f64) as i32;
            let y_end = (cy + radius).min(buffer_height as f64) as i32;

            for py in y_start..y_end {
                for px in x_start..x_end {
                    let dx = px as f64 - cx;
                    let dy = py as f64 - cy;
                    if dx * dx + dy * dy <= r2 {
                        let offset = (py as i64 * pitch + px as i64 * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = color.0; // B
                            buffer[offset + 1] = color.1; // G
                            buffer[offset + 2] = color.2; // R
                            buffer[offset + 3] = color.3; // A
                        }
                    }
                }
            }
        }

        fn draw_circle_outline(
            &self,
            buffer: &mut [u8],
            buffer_width: i64,
            buffer_height: i64,
            pitch: i64,
            cx: f64,
            cy: f64,
            radius: f64,
            color: (u8, u8, u8, u8),
        ) {
            let thickness = 2.0;
            let outer_r2 = radius * radius;
            let inner_r2 = (radius - thickness) * (radius - thickness);

            let x_start = (cx - radius).max(0.0) as i32;
            let y_start = (cy - radius).max(0.0) as i32;
            let x_end = (cx + radius).min(buffer_width as f64) as i32;
            let y_end = (cy + radius).min(buffer_height as f64) as i32;

            for py in y_start..y_end {
                for px in x_start..x_end {
                    let dx = px as f64 - cx;
                    let dy = py as f64 - cy;
                    let dist2 = dx * dx + dy * dy;
                    if dist2 <= outer_r2 && dist2 >= inner_r2 {
                        let offset = (py as i64 * pitch + px as i64 * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = color.0; // B
                            buffer[offset + 1] = color.1; // G
                            buffer[offset + 2] = color.2; // R
                            buffer[offset + 3] = color.3; // A
                        }
                    }
                }
            }
        }

        pub fn register_atlases(&mut self, gpu_renderer: &mut GPURenderer) {
            // Background (dark gray)
            if self.background_atlas_id.is_none() {
                let bg_pixel = vec![50u8, 50, 50, 255]; // RGBA
                let id = gpu_renderer.register_atlas(bg_pixel, 1, 1, AtlasFormat::RGBA);
                self.background_atlas_id = Some(id);
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

            // Bar foreground (green)
            if self.bar_fg_atlas_id.is_none() {
                let bar_fg_pixel = vec![0u8, 200, 0, 255];
                let id = gpu_renderer.register_atlas(bar_fg_pixel, 1, 1, AtlasFormat::RGBA);
                self.bar_fg_atlas_id = Some(id);
            }

            // Button off (gray)
            if self.button_off_atlas_id.is_none() {
                let button_off_pixel = vec![100u8, 100, 100, 255];
                let id = gpu_renderer.register_atlas(button_off_pixel, 1, 1, AtlasFormat::RGBA);
                self.button_off_atlas_id = Some(id);
            }

            // Button on (green)
            if self.button_on_atlas_id.is_none() {
                let button_on_pixel = vec![0u8, 255, 0, 255];
                let id = gpu_renderer.register_atlas(button_on_pixel, 1, 1, AtlasFormat::RGBA);
                self.button_on_atlas_id = Some(id);
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
                        color: (255, 255, 255, 255),
                    });
                    // Bottom
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: x,
                        dest_y: y + h - 1.0,
                        dest_width: w,
                        dest_height: 1.0,
                        rotation: 0.0,
                        color: (255, 255, 255, 255),
                    });
                    // Left
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: x,
                        dest_y: y,
                        dest_width: 1.0,
                        dest_height: h,
                        rotation: 0.0,
                        color: (255, 255, 255, 255),
                    });
                    // Right
                    gpu_renderer.add_command(RenderCommand::Rect {
                        texture_id: border_id,
                        dest_x: x + w - 1.0,
                        dest_y: y,
                        dest_width: 1.0,
                        dest_height: h,
                        rotation: 0.0,
                        color: (255, 255, 255, 255),
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
                            color: (255, 255, 255, 255),
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
                                    color: (255, 255, 255, 255),
                                });
                            } else {
                                gpu_renderer.add_command(RenderCommand::Rect {
                                    texture_id: bar_fg_id,
                                    dest_x: center + (value as f64) * vis_w / 2.0,
                                    dest_y: vis_y,
                                    dest_width: -(value as f64) * vis_w / 2.0,
                                    dest_height: vis_h,
                                    rotation: 0.0,
                                    color: (255, 255, 255, 255),
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
                                color: (255, 255, 255, 255),
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

                for (i, name) in button_names.iter().enumerate() {
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
                                color: (255, 255, 255, 255),
                            });
                        }

                        button_x += 30.0;
                        if (i + 1) % 5 == 0 {
                            button_x = x + 10.0;
                            button_row_y += 25.0;
                        }
                    }
                }
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            // GPU only - no CPU rendering
            let _ = (buffer, buffer_width, buffer_height, pitch);
        }
    }
});
