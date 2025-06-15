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

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if let Some(rect) = &mut self.rect {
                let (x, y, w, h) = rect.bounds();

                // Draw background
                let x_start = x.max(0.0) as i32;
                let y_start = y.max(0.0) as i32;
                let x_end = (x + w).min(buffer_width as f64) as i32;
                let y_end = (y + h).min(buffer_height as f64) as i32;

                for py in y_start..y_end {
                    for px in x_start..x_end {
                        let offset = (py as i64 * pitch + px as i64 * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = 40; // B
                            buffer[offset + 1] = 40; // G
                            buffer[offset + 2] = 40; // R
                            buffer[offset + 3] = 255; // A
                        }
                    }
                }

                // Draw border
                for px in x_start..x_end {
                    // Top border
                    if y_start >= 0 {
                        let offset = (y_start as i64 * pitch + px as i64 * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = 128;
                            buffer[offset + 1] = 128;
                            buffer[offset + 2] = 128;
                            buffer[offset + 3] = 255;
                        }
                    }
                    // Bottom border
                    if y_end - 1 < buffer_height as i32 {
                        let offset = ((y_end - 1) as i64 * pitch + px as i64 * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = 128;
                            buffer[offset + 1] = 128;
                            buffer[offset + 2] = 128;
                            buffer[offset + 3] = 255;
                        }
                    }
                }

                for py in y_start..y_end {
                    // Left border
                    if x_start >= 0 {
                        let offset = (py as i64 * pitch + x_start as i64 * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = 128;
                            buffer[offset + 1] = 128;
                            buffer[offset + 2] = 128;
                            buffer[offset + 3] = 255;
                        }
                    }
                    // Right border
                    if x_end - 1 < buffer_width as i32 {
                        let offset = (py as i64 * pitch + (x_end - 1) as i64 * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = 128;
                            buffer[offset + 1] = 128;
                            buffer[offset + 2] = 128;
                            buffer[offset + 3] = 255;
                        }
                    }
                }

                // Render labels
                let mut label_y = y + 10.0;
                for label in &mut self.labels {
                    label.set_x(x + 10.0);
                    label.set_y(label_y);
                    label.render(buffer, buffer_width, buffer_height, pitch);
                    label_y += 20.0;
                }

                // Collect axis visualization data first
                label_y = y + 60.0;
                let mut axis_viz_data = Vec::new();
                for i in 0..self.axis_labels.len() {
                    let vis_x = x + 100.0;
                    let vis_y = label_y + i as f64 * 20.0 - 5.0;
                    let vis_w = 100.0;
                    let vis_h = 10.0;

                    let value = self.axes[i];
                    let normalized = if i < 4 {
                        // Sticks: -1 to 1, draw from center
                        let center = vis_x + vis_w / 2.0;
                        if value >= 0.0 {
                            (center, (value as f64) * vis_w / 2.0)
                        } else {
                            (center + (value as f64) * vis_w / 2.0, -(value as f64) * vis_w / 2.0)
                        }
                    } else {
                        // Triggers: 0 to 1, draw from left
                        (vis_x, (value as f64) * vis_w)
                    };

                    axis_viz_data.push((vis_x, vis_y, vis_w, vis_h, normalized));
                }

                // Render axis labels
                for (i, label) in self.axis_labels.iter_mut().enumerate() {
                    label.set_x(x + 10.0);
                    label.set_y(label_y + i as f64 * 20.0);
                    label.render(buffer, buffer_width, buffer_height, pitch);
                }

                // Draw axis visualizations
                for (vis_x, vis_y, vis_w, vis_h, normalized) in axis_viz_data {
                    // Background bar
                    self.draw_rect(
                        buffer,
                        buffer_width,
                        buffer_height,
                        pitch,
                        vis_x as i32,
                        vis_y as i32,
                        vis_w as i32,
                        vis_h as i32,
                        (64, 64, 64, 255),
                    );

                    // Value bar
                    self.draw_rect(
                        buffer,
                        buffer_width,
                        buffer_height,
                        pitch,
                        normalized.0 as i32,
                        vis_y as i32,
                        normalized.1 as i32,
                        vis_h as i32,
                        (0, 255, 0, 255),
                    );
                }

                // Draw stick positions visually
                let stick_size = 60.0;
                let left_stick_x = x + 10.0;
                let left_stick_y = y + 200.0;
                let right_stick_x = x + 100.0;
                let right_stick_y = y + 200.0;

                // Left stick
                self.draw_circle_outline(
                    buffer,
                    buffer_width,
                    buffer_height,
                    pitch,
                    left_stick_x + stick_size / 2.0,
                    left_stick_y + stick_size / 2.0,
                    stick_size / 2.0,
                    (128, 128, 128, 255),
                );
                self.draw_circle(
                    buffer,
                    buffer_width,
                    buffer_height,
                    pitch,
                    left_stick_x + stick_size / 2.0 + (self.axes[0] as f64) * stick_size / 2.0,
                    left_stick_y + stick_size / 2.0 + (self.axes[1] as f64) * stick_size / 2.0,
                    5.0,
                    (0, 255, 0, 255),
                );

                // Right stick
                self.draw_circle_outline(
                    buffer,
                    buffer_width,
                    buffer_height,
                    pitch,
                    right_stick_x + stick_size / 2.0,
                    right_stick_y + stick_size / 2.0,
                    stick_size / 2.0,
                    (128, 128, 128, 255),
                );
                self.draw_circle(
                    buffer,
                    buffer_width,
                    buffer_height,
                    pitch,
                    right_stick_x + stick_size / 2.0 + (self.axes[2] as f64) * stick_size / 2.0,
                    right_stick_y + stick_size / 2.0 + (self.axes[3] as f64) * stick_size / 2.0,
                    5.0,
                    (0, 255, 0, 255),
                );

                // Draw buttons
                let button_names = [
                    "A", "B", "X", "Y", "Back", "Guide", "Start", "LS", "RS", "LB", "RB", "Up", "Down", "Left", "Right",
                ];
                let mut button_x = x + 10.0;
                let mut button_y = y + 280.0;
                for (i, &pressed) in self.buttons.iter().enumerate() {
                    if i < button_names.len() {
                        let color = if pressed { (0, 255, 0, 255) } else { (64, 64, 64, 255) };
                        self.draw_rect(
                            buffer,
                            buffer_width,
                            buffer_height,
                            pitch,
                            button_x as i32,
                            button_y as i32,
                            25,
                            20,
                            color,
                        );

                        // Draw button label
                        let mut label = TextRenderer::new()
                            .with_text(button_names[i].to_string())
                            .with_x(button_x + 2.0)
                            .with_y(button_y + 2.0)
                            .with_color((255, 255, 255, 255));
                        label.render(buffer, buffer_width, buffer_height, pitch);

                        button_x += 30.0;
                        if (i + 1) % 5 == 0 {
                            button_x = x + 10.0;
                            button_y += 25.0;
                        }
                    }
                }
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
    }
});
