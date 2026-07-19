use crate::message::{Conversation, Message, Role};
use crate::provider;
use crate::tools::ToolRegistry;
use ansi_to_tui::IntoText;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;
use termimad::MadSkin;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

enum LLMEvent {
    Token(String),
    Done(Message),
    Error(String),
}

pub struct App {
    conversation: Conversation,
    tool_registry: ToolRegistry,
    input: String,
    cursor_pos: usize,
    scroll: u16,
    auto_scroll: bool,
    streaming: bool,
    current_response: String,
    event_rx: Receiver<LLMEvent>,
    event_tx: Sender<LLMEvent>,
}

impl App {
    pub fn new(conversation: Conversation, tool_registry: ToolRegistry) -> Self {
        let (event_tx, event_rx) = channel::<LLMEvent>();
        Self {
            conversation,
            tool_registry,
            input: String::new(),
            cursor_pos: 0,
            scroll: 0,
            auto_scroll: true,
            streaming: false,
            current_response: String::new(),
            event_rx,
            event_tx,
        }
    }

    pub fn run(
        &mut self,
        terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Esc => return Ok(()),
                            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                return Ok(());
                            }
                            KeyCode::Enter => {
                                if key.modifiers.contains(KeyModifiers::SHIFT)
                                    || key.modifiers.contains(KeyModifiers::ALT)
                                {
                                    self.input.insert(self.cursor_pos, '\n');
                                    self.cursor_pos += 1;
                                } else if !self.streaming && !self.input.trim().is_empty() {
                                    let content = self.input.trim().to_string();
                                    self.conversation.add_message(Role::User, &content);
                                    self.input.clear();
                                    self.cursor_pos = 0;
                                    self.auto_scroll = true;
                                    self.start_stream();
                                }
                            }
                            KeyCode::Char(c) => {
                                self.input.insert(self.cursor_pos, c);
                                self.cursor_pos += 1;
                            }
                            KeyCode::Backspace => {
                                if self.cursor_pos > 0 {
                                    self.cursor_pos -= 1;
                                    self.input.remove(self.cursor_pos);
                                }
                            }
                            KeyCode::Left => {
                                if self.cursor_pos > 0 {
                                    self.cursor_pos -= 1;
                                }
                            }
                            KeyCode::Right => {
                                if self.cursor_pos < self.input.len() {
                                    self.cursor_pos += 1;
                                }
                            }
                            KeyCode::Up => {
                                if let Some(pos) = self.input[..self.cursor_pos].rfind('\n') {
                                    self.cursor_pos = pos;
                                } else if self.cursor_pos > 0 {
                                    self.cursor_pos = 0;
                                } else {
                                    self.auto_scroll = false;
                                    self.scroll = self.scroll.saturating_sub(1);
                                }
                            }
                            KeyCode::Down => {
                                if self.cursor_pos < self.input.len() {
                                    if let Some(pos) = self.input[self.cursor_pos + 1..].find('\n')
                                    {
                                        self.cursor_pos = self.cursor_pos + 1 + pos;
                                    } else {
                                        self.cursor_pos = self.input.len();
                                    }
                                } else {
                                    self.scroll = self.scroll.saturating_add(1);
                                }
                            }
                            KeyCode::End => {
                                self.auto_scroll = true;
                            }
                            _ => {}
                        }
                    }
                }
            }

            while let Ok(event) = self.event_rx.try_recv() {
                match event {
                    LLMEvent::Token(token) => {
                        self.current_response.push_str(&token);
                        if self.auto_scroll {
                            self.scroll = u16::MAX;
                        }
                    }
                    LLMEvent::Done(msg) => {
                        self.streaming = false;
                        self.conversation
                            .add_assistant_message(&msg.content, msg.tool_calls.clone());
                        self.current_response.clear();

                        if !msg.tool_calls.is_empty() {
                            for tc in &msg.tool_calls {
                                let result = self.tool_registry.execute(tc);
                                match result {
                                    Ok(output) => {
                                        self.conversation.add_tool_message(&tc.id, &output);
                                    }
                                    Err(e) => {
                                        self.conversation
                                            .add_tool_message(&tc.id, &format!("Error: {}", e));
                                    }
                                }
                            }
                            self.start_stream();
                        }
                    }
                    LLMEvent::Error(e) => {
                        self.streaming = false;
                        self.current_response.clear();
                        self.conversation
                            .add_message(Role::Assistant, &format!("Error: {}", e));
                    }
                }
            }
        }
    }

    fn start_stream(&mut self) {
        self.streaming = true;
        self.current_response.clear();
        let conv = self.conversation.clone();
        let registry = self.tool_registry.clone();
        let tx = self.event_tx.clone();

        tokio::spawn(async move {
            let result = provider::stream_openai_api(&conv, &registry, |token| {
                let _ = tx.send(LLMEvent::Token(token.to_string()));
            })
            .await;

            match result {
                Ok(msg) => {
                    let _ = tx.send(LLMEvent::Done(msg));
                }
                Err(e) => {
                    let _ = tx.send(LLMEvent::Error(e.to_string()));
                }
            }
        });
    }

    fn draw(&mut self, frame: &mut Frame) {
        let input_area_width = frame.area().width.saturating_sub(2) as usize;

        // --- manually wrap input for consistent height & cursor ---
        let wrapped_input = wrap_hard(&self.input, input_area_width);
        let input_lines = wrapped_input.len().max(1);
        let input_height = (input_lines as u16 + 2).max(3);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Fill(1), Constraint::Length(input_height)])
            .split(frame.area());

        let chat_area = chunks[0];
        let input_area = chunks[1];
        let available_width = chat_area.width as usize;
        const USER_BG: Color = Color::Rgb(35, 35, 35);

        // --- chat area ---
        let mut lines: Vec<Line> = Vec::new();

        for msg in self.conversation.get_messages() {
            match msg.role {
                Role::System => continue,
                Role::User => {
                    let user_width = available_width.saturating_sub(4);
                    let padding_line =
                        Line::from(" ".repeat(available_width)).style(Style::default().bg(USER_BG));

                    lines.push(padding_line.clone());

                    for line in wrap_hard(&msg.content.trim(), user_width) {
                        let line_width = line.width();
                        let right_pad = available_width.saturating_sub(line_width + 2);

                        // Формируем текст строки без индивидуальных стилей у Span
                        lines.push(
                            Line::from(vec![
                                Span::raw("  "),
                                Span::raw(line),
                                Span::raw(" ".repeat(right_pad)),
                            ])
                            .style(Style::default().bg(USER_BG)), // Красит всю строку целиком, включая пробелы
                        );
                    }
                    lines.push(padding_line);
                    lines.push(Line::from(""));
                }
                Role::Assistant => {
                    let rendered = render_markdown(&msg.content.trim_start());
                    for mut line in rendered.lines {
                        line.spans.insert(0, Span::raw("  "));
                        lines.push(line);
                    }
                    if !msg.tool_calls.is_empty() {
                        for tc in &msg.tool_calls {
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(
                                    format!(
                                        "{}: {}",
                                        tc.name,
                                        tc.arguments.chars().take(40).collect::<String>()
                                    ),
                                    Style::default().fg(Color::Yellow),
                                ),
                            ]));
                        }
                    }
                    lines.push(Line::from(""));
                }
                Role::Tool => {}
            }
        }

        if self.streaming && !self.current_response.is_empty() {
            let rendered = render_markdown(&self.current_response.trim_start());
            for mut line in rendered.lines {
                line.spans.insert(0, Span::raw("  "));
                lines.push(line);
            }
        }

        let visible_height = chat_area.height;
        let available_width = chat_area.width as usize;

        let total_visual_lines: u16 = lines
            .iter()
            .map(|line| {
                let w = line.width() as usize;
                if w == 0 {
                    1u16
                } else {
                    ((w - 1) / available_width.max(1) + 1) as u16
                }
            })
            .sum();

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text);

        let max_scroll = total_visual_lines.saturating_sub(visible_height);
        if self.auto_scroll {
            self.scroll = max_scroll;
        } else {
            self.scroll = self.scroll.min(max_scroll);
        }
        frame.render_widget(paragraph.scroll((self.scroll, 0)), chat_area);

        // --- input area (render pre-wrapped lines, no Paragraph wrap) ---
        let input_title = if self.streaming { "streaming..." } else { "" };
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

        // --- cursor position based on hard-wrapped text ---
        let (cursor_x, cursor_y) = {
            let mut y = 0u16;
            let mut x = 0u16;
            let mut chars_seen = 0usize;

            for line in self.input.lines() {
                let _line_w = line.width() as usize;
                let mut col_in_line = 0usize;
                for ch in line.chars() {
                    if chars_seen == self.cursor_pos {
                        x = col_in_line as u16;
                        break;
                    }
                    col_in_line += ch.width().unwrap_or(0);
                    if col_in_line > input_area_width {
                        y += 1;
                        col_in_line = ch.width().unwrap_or(0);
                    }
                    chars_seen += ch.len_utf8();
                }

                if chars_seen == self.cursor_pos {
                    x = col_in_line as u16;
                    break;
                }

                // newline
                if chars_seen < self.cursor_pos {
                    chars_seen += 1; // '\n'
                    y += 1;
                }
            }

            if chars_seen == self.cursor_pos && self.input.ends_with('\n') {
                x = 0;
            }

            (input_area.x + x, input_area.y + y + 1)
        };
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}

/// Hard-wrap a string at character width boundaries.
fn wrap_hard(text: &str, width: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for raw_line in text.split('\n') {
        let mut current = String::new();
        let mut current_width = 0usize;
        for ch in raw_line.chars() {
            let w = ch.width().unwrap_or(0);
            if current_width + w > width && width > 0 {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            current.push(ch);
            current_width += w;
        }
        if current.is_empty() && !lines.is_empty() {
            lines.push(String::new());
        } else {
            lines.push(current);
        }
    }
    lines
}

pub fn render_markdown(text: &str) -> Text<'static> {
    if text.trim().is_empty() {
        return Text::default();
    }
    let skin = MadSkin::default();
    let ct = skin.term_text(text);
    let ansi_string = format!("{}", ct);
    ansi_string
        .into_text()
        .unwrap_or_else(|_| Text::from(text.to_string()))
}
