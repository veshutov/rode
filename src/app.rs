use crate::message::{Conversation, Message, Role};
use crate::provider;
use crate::render::render_markdown;
use crate::tools::ToolRegistry;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

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
                                if !self.streaming && !self.input.trim().is_empty() {
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
                                self.auto_scroll = false;
                                self.scroll = self.scroll.saturating_sub(1);
                            }
                            KeyCode::Down => {
                                self.scroll = self.scroll.saturating_add(1);
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
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(frame.area());

        let chat_area = chunks[0];
        let input_area = chunks[1];

        let mut lines: Vec<Line> = Vec::new();

        for msg in self.conversation.get_messages() {
            match msg.role {
                Role::System => continue,
                Role::User => {
                    lines.push(Line::from(Span::styled(
                        "👤 You",
                        Style::default().fg(Color::Cyan),
                    )));
                    for line in msg.content.lines() {
                        lines.push(Line::from(line));
                    }
                    lines.push(Line::from(""));
                }
                Role::Assistant => {
                    lines.push(Line::from(Span::styled(
                        "🤖 Assistant",
                        Style::default().fg(Color::Green),
                    )));
                    let rendered = render_markdown(&msg.content);
                    lines.extend(rendered.lines);
                    if !msg.tool_calls.is_empty() {
                        for tc in &msg.tool_calls {
                            lines.push(Line::from(Span::styled(
                                format!("  🔧 {}: {}", tc.name, tc.arguments),
                                Style::default().fg(Color::Yellow),
                            )));
                        }
                    }
                    lines.push(Line::from(""));
                }
                Role::Tool => {
                    // lines.push(Line::from(Span::styled(
                    //     "📊 Result",
                    //     Style::default().fg(Color::Yellow),
                    // )));
                    // for line in msg.content.lines() {
                    //     lines.push(Line::from(line));
                    // }
                    // lines.push(Line::from(""));
                }
            }
        }

        if self.streaming && !self.current_response.is_empty() {
            lines.push(Line::from(Span::styled(
                "🤖 Assistant",
                Style::default().fg(Color::Green),
            )));
            let rendered = render_markdown(&self.current_response);
            lines.extend(rendered.lines);
            lines.push(Line::from(""));
        }

        let visible_height = chat_area.height.saturating_sub(2);
        let available_width = chat_area.width.saturating_sub(2) as usize;

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
        let paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Chat"));

        let max_scroll = total_visual_lines.saturating_sub(visible_height);

        if self.auto_scroll {
            self.scroll = max_scroll;
        } else {
            self.scroll = self.scroll.min(max_scroll);
        }

        frame.render_widget(paragraph.scroll((self.scroll, 0)), chat_area);

        let input_title = if self.streaming {
            "Input (streaming...)"
        } else {
            "Input"
        };
        let input_paragraph = Paragraph::new(self.input.as_str())
            .block(Block::default().borders(Borders::ALL).title(input_title));
        frame.render_widget(input_paragraph, input_area);

        let cursor_x = input_area.x + self.cursor_pos as u16 + 1;
        let cursor_y = input_area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
