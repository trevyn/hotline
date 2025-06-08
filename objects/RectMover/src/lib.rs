use hotline::HotlineObject;

hotline::object!({
    #[derive(Clone, Default)]
    pub struct RectMover {
        target: Option<Rect>,
    }

    impl RectMover {
        pub fn set_target(&mut self, rect: Rect) {
            self.target = Some(rect);
        }

        pub fn update(&mut self, mouse_x: f64, mouse_y: f64) {
            if let Some(ref mut rect) = self.target {
                let (cx, cy) = rect.center();
                let dx = mouse_x - cx;
                let dy = mouse_y - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                if dist > 1.0 {
                    let step = 0.5;
                    rect.move_by(dx / dist * step, dy / dist * step);
                }
            }
        }
    }
});
