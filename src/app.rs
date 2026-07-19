use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use std::time::Duration;

use crate::state::{AppState, LLMEvent};

pub struct App {
    state: AppState,
}

impl App {
    pub fn new(
        conversation: crate::message::Conversation,
        tool_registry: crate::tools::ToolRegistry,
    ) -> (Self, std::sync::mpsc::Receiver<LLMEvent>) {
        let (state, event_rx) = AppState::new(conversation, tool_registry);
        let app = Self { state };
        (app, event_rx)
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        event_rx: &std::sync::mpsc::Receiver<LLMEvent>,
    ) -> anyhow::Result<()> {
        loop {
            terminal.draw(|frame| {
                self.state.tui.render(
                    frame,
                    &mut self.state.conversation.get_messages(),
                    &self.state.current_response,
                    self.state.streaming,
                )
            })?;

            if event::poll(Duration::from_millis(25))? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Press {
                            if self.handle_key(key)? {
                                return Ok(());
                            }
                        }
                    }
                    Event::Paste(text) => {
                        for ch in text.chars() {
                            if ch == '\n' {
                                self.state.tui.input.insert_newline();
                            } else {
                                self.state.tui.input.insert(ch);
                            }
                        }
                    }
                    _ => {}
                }
            }

            while let Ok(event) = event_rx.try_recv() {
                let followups = self.state.handle_llm_event(event);
                if !followups.is_empty() {
                    self.state.start_stream();
                }
            }
        }
    }

    fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> anyhow::Result<bool> {
        match key.code {
            KeyCode::Esc => return Ok(true),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.state.streaming {
                    self.state.cancel();
                } else {
                    return Ok(true);
                }
            }
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT)
                {
                    self.state.tui.input.insert_newline();
                } else if !self.state.streaming && !self.state.tui.input.is_empty() {
                    let content = self.state.tui.input.take().trim().to_string();
                    if content == "/clear" {
                        self.state.clear();
                    } else {
                        self.state.submit_user_message(&content);
                    }
                }
            }
            KeyCode::Char('u')
                if key.modifiers.contains(KeyModifiers::SUPER)
                    || key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.state.tui.input.delete_to_start_of_line();
            }
            KeyCode::Char(c) => {
                self.state.tui.input.insert(c);
            }
            KeyCode::Backspace => {
                if key.modifiers.contains(KeyModifiers::SUPER) {
                    self.state.tui.input.delete_to_start_of_line();
                } else if key.modifiers.contains(KeyModifiers::ALT) {
                    self.state.tui.input.delete_word_before_cursor();
                } else {
                    self.state.tui.input.backspace();
                }
            }
            KeyCode::Left => {
                self.state.tui.input.move_left();
            }
            KeyCode::Right => {
                self.state.tui.input.move_right();
            }
            KeyCode::Up => {
                if !self.state.tui.input.move_up() {
                    self.state.tui.scroll.scroll_up();
                }
            }
            KeyCode::Down => {
                if !self.state.tui.input.move_down() {
                    self.state.tui.scroll.scroll_down();
                }
            }
            KeyCode::End => {
                self.state.tui.scroll.scroll_to_end();
            }
            _ => {}
        }
        Ok(false)
    }
}
