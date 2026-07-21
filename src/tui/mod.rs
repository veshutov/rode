use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    agent::message::Message,
    tui::{input::InputBuffer, line::MessageLinesBuilder, scroll::Scroll},
};

pub mod input;
pub mod line;
pub mod scroll;
mod utils;

pub enum TUICommand {
    Submit(String),
    Cancel,
    Exit,
}

pub struct Tui {
    input: InputBuffer,
    scroll: Scroll,
    line_builder: MessageLinesBuilder,
}

impl Tui {
    pub fn new() -> Self {
        Self {
            input: InputBuffer::new(),
            line_builder: MessageLinesBuilder::new(),
            scroll: Scroll::new(),
        }
    }

    pub fn on_event(&mut self, event: &Event, streaming: bool) -> Option<TUICommand> {
        match event {
            Event::Key(key) => {
                if key.kind == KeyEventKind::Press {
                    return self.handle_key(key, streaming);
                }
            }
            Event::Paste(text) => {
                for ch in text.chars() {
                    if ch == '\n' {
                        self.input.insert_newline();
                    } else {
                        self.input.insert(ch);
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn handle_key(&mut self, key: &KeyEvent, streaming: bool) -> Option<TUICommand> {
        match key.code {
            KeyCode::Esc => return Some(TUICommand::Exit),
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT)
                {
                    self.input.insert_newline();
                } else if !streaming && !self.input.is_empty() {
                    let content = self.input.take().trim().to_string();
                    self.scroll.set_auto();
                    return Some(TUICommand::Submit(content));
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Some(TUICommand::Cancel);
            }
            // macOS terminals translate Cmd+Left/Right into Ctrl+A/E
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.move_to_start_of_line();
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.move_to_end_of_line();
            }
            // macOS terminals translate Cmd+Backspace into Ctrl+U
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.delete_to_start_of_line();
            }
            // Some terminals send Option+Left/Right as Alt+B/F
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.input.move_word_left();
            }
            KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::ALT) => {
                self.input.move_word_right();
            }
            KeyCode::Char(c) => {
                self.input.insert(c);
            }
            KeyCode::Backspace => {
                if key.modifiers.contains(KeyModifiers::ALT) {
                    self.input.delete_word_before_cursor();
                } else {
                    self.input.backspace();
                }
            }
            KeyCode::Left => {
                if key.modifiers.contains(KeyModifiers::ALT) {
                    self.input.move_word_left();
                } else {
                    self.input.move_left();
                }
            }
            KeyCode::Right => {
                if key.modifiers.contains(KeyModifiers::ALT) {
                    self.input.move_word_right();
                } else {
                    self.input.move_right();
                }
            }
            KeyCode::Up => {
                if !self.input.move_up() {
                    self.scroll.scroll_up();
                }
            }
            KeyCode::Down => {
                if !self.input.move_down() {
                    self.scroll.scroll_down();
                }
            }
            KeyCode::End => {
                self.scroll.set_auto();
            }
            _ => {}
        }
        None
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        messages: &[Message],
        current_response: &str,
        streaming: bool,
        status_message: &str,
        context_tokens: Option<u32>,
    ) {
        let input_area_width = frame.area().width.saturating_sub(2) as usize;
        let wrapped_input = self.input.wrapped(input_area_width);
        let input_lines = wrapped_input.len().max(1);
        let input_height = (input_lines as u16 + 2).max(3);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(input_height),
                Constraint::Length(1),
            ])
            .split(frame.area());

        let chat_area = chunks[0];
        let input_area = chunks[1];
        let footer_area = chunks[2];

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

        let input_title = if streaming {
            "working..."
        } else if !status_message.is_empty() {
            status_message.lines().next().unwrap_or("")
        } else {
            ""
        };
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

        if let Some(tokens) = context_tokens {
            let footer = format!(" context: {} tokens ", tokens);
            frame.render_widget(
                Paragraph::new(Line::from(footer)).style(ratatui::style::Style::default().dim()),
                footer_area,
            );
        }
    }

    pub fn reset(&mut self) {
        self.scroll.reset();
        self.line_builder.clear_cache();
    }
}
