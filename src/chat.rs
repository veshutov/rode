use ratatui::{
    layout::Rect,
    text::Line,
    widgets::{Block, Paragraph, Widget},
};

pub struct ChatWidget<'a> {
    messages: &'a [String],
    block: Option<Block<'a>>,
    scroll: u16,
}

impl<'a> ChatWidget<'a> {
    pub fn new(messages: &'a [String], scroll: u16) -> Self {
        Self {
            messages,
            block: None,
            scroll,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    fn build_lines(&self, height: usize) -> Vec<Line<'a>> {
        // Convert messages → lines
        let mut lines: Vec<Line> = self
            .messages
            .iter()
            .map(|m| Line::from(m.as_str()))
            .collect();

        // Keep only last N that fit
        if lines.len() > height {
            lines = lines[lines.len() - height..].to_vec();
        }

        // Pad top so content sticks to bottom
        let padding = height.saturating_sub(lines.len());
        let mut padded = Vec::with_capacity(height);

        padded.extend(std::iter::repeat_n(Line::from(""), padding));
        padded.extend(lines);

        padded
    }
}

impl<'a> Widget for ChatWidget<'a> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let inner_area = if let Some(block) = &self.block {
            block.inner(area)
        } else {
            area
        };

        let height = inner_area.height as usize;
        let lines = self.build_lines(height);

        let height = inner_area.height as usize;

        // max scroll = how many lines can be skipped
        let max_scroll = lines.len().saturating_sub(height) as u16;

        // clamp scroll so we never go out of bounds
        let scroll = self.scroll.min(max_scroll);

        let paragraph = Paragraph::new(lines).scroll((scroll, 0));

        if let Some(block) = self.block {
            block.render(area, buf);
        }

        paragraph.render(inner_area, buf);
    }
}
