use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    message::Message,
    tui::{input::InputBuffer, line::MessageLinesBuilder, scroll::Scroll},
};

pub mod input;
pub mod line;
pub mod scroll;
mod utils;

pub struct TUI {
    pub input: InputBuffer,
    pub scroll: Scroll,
    line_builder: MessageLinesBuilder,
}

impl TUI {
    pub fn new() -> Self {
        Self {
            input: InputBuffer::new(),
            line_builder: MessageLinesBuilder::new(),
            scroll: Scroll::new(),
        }
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        messages: &[Message],
        current_response: &str,
        streaming: bool,
    ) {
        let input_area_width = frame.area().width.saturating_sub(2) as usize;
        let wrapped_input = self.input.wrapped(input_area_width);
        let input_lines = wrapped_input.len().max(1);
        let input_height = (input_lines as u16 + 2).max(3);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(input_height)])
            .split(frame.area());

        let chat_area = chunks[0];
        let input_area = chunks[1];

        let lines = self.line_builder.build(
            messages,
            chat_area.width as usize,
            current_response,
            streaming,
        );
        let scroll = self.scroll.update_scroll(lines.len(), chat_area.height);
        let text = Text::from(lines);
        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph.scroll((scroll, 0)), chat_area);

        let input_title = if streaming { "working..." } else { "" };
        let input_text = Text::from(
            wrapped_input
                .into_iter()
                .map(Line::from)
                .collect::<Vec<_>>(),
        );
        let input_paragraph = Paragraph::new(input_text).block(
            Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .title(input_title),
        );
        frame.render_widget(input_paragraph, input_area);

        let (cursor_x, cursor_y) = self.input.cursor_xy(input_area_width);
        frame.set_cursor_position((input_area.x + cursor_x, input_area.y + cursor_y + 1));
    }

    pub fn reset(&mut self) {
        self.scroll.reset();
        self.line_builder.clear_cache();
    }
}
