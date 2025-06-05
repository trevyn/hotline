use hotline::HotlineObject;

#[derive(Clone, Default)]
struct Menu {
    label: String,
    items: Vec<String>,
}

hotline::object!({
    #[derive(Default)]
    pub struct MenuBar {
        menus: Vec<Menu>,
        open_menu: Option<usize>,
        #[default(20.0)]
        height: f64,
    }

    impl MenuBar {
        pub fn add_menu(&mut self, label: String) -> i64 {
            self.menus.push(Menu { label, items: Vec::new() });
            (self.menus.len() - 1) as i64
        }

        pub fn add_menu_item(&mut self, index: i64, label: String) {
            if let Some(menu) = self.menus.get_mut(index as usize) {
                menu.items.push(label);
            }
        }

        pub fn bar_height(&mut self) -> f64 {
            self.height
        }

        pub fn handle_mouse_down(&mut self, x: f64, y: f64) {
            let item_width = 80.0;
            let item_height = 20.0;
            if y < self.height {
                let index = (x / item_width).floor() as usize;
                if index < self.menus.len() {
                    if self.open_menu == Some(index) {
                        self.open_menu = None;
                    } else {
                        self.open_menu = Some(index);
                    }
                } else {
                    self.open_menu = None;
                }
            } else if let Some(menu_index) = self.open_menu {
                let rel_y = y - self.height;
                let item_index = (rel_y / item_height).floor() as usize;
                if item_index < self.menus[menu_index].items.len() {
                    let item_label = self.menus[menu_index].items[item_index].clone();
                    println!("Selected menu item: {} -> {}", self.menus[menu_index].label, item_label);
                }
                self.open_menu = None;
            } else {
                self.open_menu = None;
            }
        }

        pub fn render(&mut self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) {
            let bar_height = self.height as usize;
            for y in 0..bar_height.min(buffer_height as usize) {
                for x in 0..buffer_width as usize {
                    let offset = y * pitch as usize + x * 4;
                    if offset + 3 < buffer.len() {
                        buffer[offset] = 60;
                        buffer[offset + 1] = 60;
                        buffer[offset + 2] = 60;
                        buffer[offset + 3] = 255;
                    }
                }
            }

            let item_width = 80usize;
            let item_height = 20usize;
            if let Some(open) = self.open_menu {
                let menu = &self.menus[open];
                for (i, _) in menu.items.iter().enumerate() {
                    let y_start = bar_height + i * item_height;
                    let y_end = y_start + item_height;
                    for y in y_start..y_end {
                        if y >= buffer_height as usize {
                            break;
                        }
                        for x in (open * item_width)..((open + 1) * item_width).min(buffer_width as usize) {
                            let offset = y * pitch as usize + x * 4;
                            if offset + 3 < buffer.len() {
                                buffer[offset] = 80;
                                buffer[offset + 1] = 80;
                                buffer[offset + 2] = 80;
                                buffer[offset + 3] = 255;
                            }
                        }
                    }
                }
            }
        }
    }
});
