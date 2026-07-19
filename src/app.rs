use crate::input::InputBuffer;
use crate::state::{AppState, LLMEvent};
use crate::ui;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use std::time::Duration;

pub struct App {
    state: AppState,
    input: InputBuffer,
}

impl App {
    pub fn new(conversation: crate::message::Conversation, tool_registry: crate::tools::ToolRegistry) -> (Self, std::sync::mpsc::Receiver<LLMEvent>) {
        let (state, event_rx) = AppState::new(conversation, tool_registry);
        let app = Self {
            state,
            input: InputBuffer::new(),
        };
        (app, event_rx)
    }

    pub fn run(
        &mut self,
        terminal: &mut Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
        event_rx: &std::sync::mpsc::Receiver<LLMEvent>,
    ) -> anyhow::Result<()> {
        loop {
            terminal.draw(|frame| ui::draw(frame, &self.state, &self.input))?;

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        if self.handle_key(key)? {
                            return Ok(());
                        }
                    }
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
                return Ok(true);
            }
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::SHIFT)
                    || key.modifiers.contains(KeyModifiers::ALT)
                {
                    self.input.insert_newline();
                } else if !self.state.streaming && !self.input.is_empty() {
                    let content = self.input.take().trim().to_string();
                    self.state.submit_user_message(&content);
                }
            }
            KeyCode::Char(c) => {
                self.input.insert(c);
            }
            KeyCode::Backspace => {
                self.input.backspace();
            }
            KeyCode::Left => {
                self.input.move_left();
            }
            KeyCode::Right => {
                self.input.move_right();
            }
            KeyCode::Up => {
                if self.input.content().is_empty() || self.input.cursor_xy(1).1 == 0 {
                    self.state.scroll_up();
                } else {
                    self.input.move_up();
                }
            }
            KeyCode::Down => {
                self.input.move_down();
            }
            KeyCode::End => {
                self.state.scroll_to_end();
            }
            _ => {}
        }
        Ok(false)
    }
}
