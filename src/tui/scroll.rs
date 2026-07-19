pub struct Scroll {
    current: u16,
    auto: bool,
}

impl Scroll {
    pub fn new() -> Self {
        Self {
            current: 0,
            auto: true,
        }
    }

    pub fn update_scroll(&mut self, line_count: usize, visible_height: u16) -> u16 {
        let total_lines = line_count as u16;
        let max_scroll = total_lines.saturating_sub(visible_height);
        self.current = if self.auto {
            max_scroll
        } else {
            self.current.min(max_scroll)
        };
        self.current
    }

    pub fn scroll_up(&mut self) {
        self.auto = false;
        self.current = self.current.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.current = self.current.saturating_add(1);
    }

    pub fn scroll_to_end(&mut self) {
        self.auto = true;
    }

    pub fn set_auto(&mut self, auto: bool) {
        self.auto = auto;
    }

    pub fn reset(&mut self) {
        self.current = 0;
        self.auto = true;
    }
}
