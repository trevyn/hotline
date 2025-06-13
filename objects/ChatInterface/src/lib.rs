#[cfg(test)]
mod test_send_sync;

hotline::object!({
    #[derive(Default)]
    pub struct ChatInterface {
        bounds: Option<Rect>,

        #[setter]
        history_area: Option<TextArea>,

        #[setter]
        input_area: Option<TextArea>,

        #[setter]
        anthropic_client: Option<AnthropicClient>,

        conversation: String,
        #[default(100.0)]
        input_height: f64,
        #[default(2.0)]
        separator_height: f64,

        waiting_for_response: bool,
    }

    impl ChatInterface {
        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            if let Some(ref bounds) = self.bounds {
                let mut bounds_clone = bounds.clone();
                let (x, y, w, h) = bounds_clone.bounds();

                // render history area
                if let Some(ref mut history) = self.history_area {
                    let mut history_bounds = Rect::new();
                    history_bounds.initialize(x, y, w, h - self.input_height - self.separator_height);
                    history.set_rect(history_bounds);
                    history.render(buffer, buffer_width, buffer_height, pitch);
                }

                // render separator
                let sep_y = y + h - self.input_height - self.separator_height;
                let x_start = x.max(0.0) as u32;
                let x_end = (x + w).min(buffer_width as f64) as u32;
                let y_start = sep_y.max(0.0) as u32;
                let y_end = (sep_y + self.separator_height).min(buffer_height as f64) as u32;

                for py in y_start..y_end {
                    for px in x_start..x_end {
                        let offset = (py * (pitch as u32) + px * 4) as usize;
                        if offset + 3 < buffer.len() {
                            buffer[offset] = 77;
                            buffer[offset + 1] = 77;
                            buffer[offset + 2] = 77;
                            buffer[offset + 3] = 255;
                        }
                    }
                }

                // render input area
                if let Some(ref mut input) = self.input_area {
                    let mut input_bounds = Rect::new();
                    input_bounds.initialize(x, y + h - self.input_height, w, self.input_height);
                    input.set_rect(input_bounds);
                    input.render(buffer, buffer_width, buffer_height, pitch);
                }
            }
        }

        pub fn set_rect(&mut self, rect: Rect) {
            self.bounds = Some(rect);
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) -> bool {
            if let Some(ref bounds) = self.bounds {
                let mut bounds_clone = bounds.clone();
                let (_bx, by, _bw, bh) = bounds_clone.bounds();
                let history_height = bh - self.input_height - self.separator_height;

                if y < by + history_height {
                    if let Some(ref mut history) = self.history_area {
                        return history.handle_mouse_down(x, y);
                    }
                } else if y > by + history_height + self.separator_height {
                    if let Some(ref mut input) = self.input_area {
                        return input.handle_mouse_down(x, y);
                    }
                }
            }
            false
        }

        pub fn handle_mouse_up(&mut self) {
            if let Some(ref mut history) = self.history_area {
                history.handle_mouse_up();
            }
            if let Some(ref mut input) = self.input_area {
                input.handle_mouse_up();
            }
        }

        pub fn handle_mouse_move(&mut self, x: f64, y: f64) {
            if let Some(ref mut history) = self.history_area {
                history.handle_mouse_move(x, y);
            }
            if let Some(ref mut input) = self.input_area {
                input.handle_mouse_move(x, y);
            }
        }

        pub fn insert_char(&mut self, ch: char) {
            if let Some(ref mut input) = self.input_area {
                if ch == '\n' {
                    self.send_message();
                } else {
                    input.insert_char(ch);
                }
            }
        }

        pub fn insert_text(&mut self, text: &str) {
            if let Some(ref mut input) = self.input_area {
                input.insert_text(text);
            }
        }

        pub fn backspace(&mut self) {
            if let Some(ref mut input) = self.input_area {
                input.backspace();
            }
        }

        pub fn move_cursor_left(&mut self) {
            if let Some(ref mut input) = self.input_area {
                input.move_cursor_left(false);
            }
        }

        pub fn move_cursor_right(&mut self) {
            if let Some(ref mut input) = self.input_area {
                input.move_cursor_right(false);
            }
        }

        pub fn handle_mouse_wheel(&mut self, x: f64, y: f64, delta_y: f64) {
            if let Some(ref bounds) = self.bounds {
                let mut bounds_clone = bounds.clone();
                let (_bx, by, _bw, bh) = bounds_clone.bounds();
                let history_height = bh - self.input_height - self.separator_height;

                // Check if mouse is over history area
                if y < by + history_height {
                    if let Some(ref mut history) = self.history_area {
                        history.add_scroll_velocity(-delta_y * 20.0);
                    }
                } else if y > by + history_height + self.separator_height {
                    // Mouse is over input area
                    if let Some(ref mut input) = self.input_area {
                        input.add_scroll_velocity(-delta_y * 20.0);
                    }
                }
            }
        }

        fn send_message(&mut self) {
            if self.waiting_for_response {
                return; // Don't send another message while waiting
            }

            if let Some(ref mut input) = self.input_area {
                let message = input.get_text();
                if !message.trim().is_empty() {
                    // append user message to conversation
                    if !self.conversation.is_empty() {
                        self.conversation.push_str("\n\n");
                    }
                    self.conversation.push_str("User: ");
                    self.conversation.push_str(&message);

                    // update history
                    if let Some(ref mut history) = self.history_area {
                        history.set_text(self.conversation.clone());
                        // scroll to bottom manually
                        let line_count = self.conversation.lines().count();
                        history.set_scroll_offset((line_count as f64 - 10.0).max(0.0) * 20.0);
                    }

                    // Send to AnthropicClient
                    if let Some(ref mut client) = self.anthropic_client {
                        self.waiting_for_response = true;
                        client.send_message(message.clone());

                        // Show thinking message
                        self.conversation.push_str("\n\nAssistant: Thinking...");
                        if let Some(ref mut history) = self.history_area {
                            history.set_text(self.conversation.clone());
                            let line_count = self.conversation.lines().count();
                            history.set_scroll_offset((line_count as f64 - 10.0).max(0.0) * 20.0);
                        }
                    } else {
                        // No client connected
                        self.conversation.push_str("\n\nAssistant: [No AnthropicClient connected]");
                        if let Some(ref mut history) = self.history_area {
                            history.set_text(self.conversation.clone());
                            let line_count = self.conversation.lines().count();
                            history.set_scroll_offset((line_count as f64 - 10.0).max(0.0) * 20.0);
                        }
                    }

                    // clear input
                    input.set_text(String::new());
                }
            }
        }

        pub fn receive_llm_response(&mut self, response: String) {
            self.waiting_for_response = false;

            // Remove "Thinking..." and add actual response
            if self.conversation.ends_with("Assistant: Thinking...") {
                self.conversation.truncate(self.conversation.len() - "Thinking...".len());
                self.conversation.push_str(&response);
            } else {
                // Fallback: just append the response
                self.conversation.push_str("\n\nAssistant: ");
                self.conversation.push_str(&response);
            }

            // Update history
            if let Some(ref mut history) = self.history_area {
                history.set_text(self.conversation.clone());
                let line_count = self.conversation.lines().count();
                history.set_scroll_offset((line_count as f64 - 10.0).max(0.0) * 20.0);
            }
        }

        pub fn initialize(&mut self) {
            if let Some(registry) = self.get_registry() {
                ::hotline::set_library_registry(registry);
            }

            // create history area (read-only)
            let mut history = TextArea::new();
            history.set_editable(false);
            history.set_show_cursor(false);
            history.set_background_color(13); // dark gray
            self.set_history_area(&history);

            // create input area
            let mut input = TextArea::new();
            input.set_editable(true);
            input.set_show_cursor(true);
            input.set_background_color(38); // slightly lighter gray
            self.set_input_area(&input);
        }

        pub fn update_scroll(&mut self) {
            if let Some(ref mut history) = self.history_area {
                history.update_scroll();
            }
            if let Some(ref mut input) = self.input_area {
                input.update_scroll();
            }
        }
    }
});
