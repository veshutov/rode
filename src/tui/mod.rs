use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{
    agent::message::Message,
    tui::{commands::CommandPopup, input::InputBuffer, line::MessageLinesBuilder, scroll::Scroll},
};

pub mod commands;
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
    command_popup: CommandPopup,
}

pub struct TuiHud {
    pub model: String,
    pub context_tokens: Option<u32>,
    pub streaming: bool,
    pub status_message: String,
    pub cwd: String,
}

impl Tui {
    pub fn new() -> Self {
        Self {
            input: InputBuffer::new(),
            line_builder: MessageLinesBuilder::new(),
            scroll: Scroll::new(),
            command_popup: CommandPopup::new(),
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
        let command_mode = self.input.is_command_mode();

        // When in command mode, Up/Down navigate the popup instead of moving
        // the cursor through multi-line input.
        if command_mode && !streaming {
            match key.code {
                KeyCode::Up => {
                    self.command_popup.move_up();
                    return None;
                }
                KeyCode::Down => {
                    let filtered = self.command_popup.filtered(self.input.as_str());
                    self.command_popup.move_down(filtered.len());
                    return None;
                }
                KeyCode::Tab => {
                    // Autocomplete: replace input with the selected command
                    let filtered = self.command_popup.filtered(self.input.as_str());
                    if !filtered.is_empty() {
                        let idx = self.command_popup.selected_index();
                        let cmd = filtered[idx.min(filtered.len() - 1)];
                        self.input.replace(&format!("/{}", cmd.name));
                    }
                    return None;
                }
                KeyCode::Enter => {
                    // Submit the selected command from the popup
                    let filtered = self.command_popup.filtered(self.input.as_str());
                    if !filtered.is_empty() {
                        let idx = self.command_popup.selected_index();
                        let cmd = filtered[idx.min(filtered.len() - 1)];
                        let content = format!("/{}", cmd.name);
                        self.input.take();
                        self.scroll.set_auto();
                        self.command_popup.reset();
                        return Some(TUICommand::Submit(content));
                    }
                }
                _ => {}
            }
        }

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
                    self.command_popup.reset();
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

        if self.input.is_command_mode() {
            let filtered = self.command_popup.filtered(self.input.as_str());
            self.command_popup.clamp(filtered.len());
        }

        None
    }

    pub fn render(
        &mut self,
        frame: &mut Frame,
        hud: &TuiHud,
        messages: &[Message],
        current_response: &str,
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
                Constraint::Length(2),
            ])
            .split(frame.area());

        let chat_area = chunks[0];
        let input_area = chunks[1];
        let footer_area = chunks[2];

        let lines = self.line_builder.build(
            messages,
            chat_area.width as usize,
            current_response,
            hud.streaming,
        );

        let scroll = self.scroll.update_scroll(lines.len(), chat_area.height);
        let text = Text::from(lines);
        let paragraph = Paragraph::new(text);
        frame.render_widget(paragraph.scroll((scroll, 0)), chat_area);

        let input_title = if hud.streaming {
            "working..."
        } else if !hud.status_message.is_empty() {
            hud.status_message.lines().next().unwrap_or("")
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

        if self.input.is_command_mode() && !hud.streaming {
            self.render_command_popup(frame, input_area);
        }

        let (cursor_x, cursor_y) = self.input.cursor_xy(input_area_width);
        frame.set_cursor_position((input_area.x + cursor_x, input_area.y + cursor_y + 1));

        let [top, bottom] =
            Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(footer_area);

        let [left, right] = Layout::horizontal([Constraint::Min(0), Constraint::Min(0)]).areas(top);

        frame.render_widget(
            Paragraph::new(Line::from(hud.cwd.clone()))
                .style(ratatui::style::Style::default().dim())
                .left_aligned(),
            left,
        );
        let model = hud.model.clone();
        frame.render_widget(
            Paragraph::new(Line::from(model))
                .style(ratatui::style::Style::default().dim())
                .right_aligned(),
            right,
        );
        if let Some(tokens) = hud.context_tokens {
            frame.render_widget(
                Paragraph::new(Line::from(format!("{}k", tokens / 1000)))
                    .style(ratatui::style::Style::default().dim())
                    .right_aligned(),
                bottom,
            );
        }
    }

    fn render_command_popup(&mut self, frame: &mut Frame, input_area: Rect) {
        let filtered_len = self.command_popup.filtered(self.input.as_str()).len();
        if filtered_len == 0 {
            return;
        }
        self.command_popup.clamp(filtered_len);

        let filtered = self.command_popup.filtered(self.input.as_str());
        let popup_height = filtered.len() as u16 + 2; // +2 for border
        let popup_width = 40u16;
        let popup_area = Rect {
            x: input_area.x + 1,
            y: input_area.y.saturating_sub(popup_height),
            width: popup_width.min(input_area.width),
            height: popup_height,
        };

        frame.render_widget(Clear, popup_area);

        let selected = self.command_popup.selected_index();
        let lines: Vec<Line> = filtered
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                let style = if i == selected {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default()
                };
                Line::from(vec![
                    Span::styled(format!("/{}", cmd.name), style),
                    Span::raw(" "),
                    Span::styled(
                        cmd.description.to_string(),
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
                .style(style)
            })
            .collect();

        let popup =
            Paragraph::new(lines).block(Block::default().title("Commands").style(Style::default()));
        frame.render_widget(popup, popup_area);
    }

    pub fn reset(&mut self) {
        self.scroll.reset();
        self.line_builder.clear_cache();
        self.command_popup.reset();
    }
}
