use crossterm::event;
use ratatui::Terminal;
use std::time::Duration;

use crate::{
    state::{AppState, LLMEvent},
    tui::{TUI, TUIEvent},
};

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
        let mut tui = TUI::new();
        loop {
            terminal.draw(|frame| {
                tui.render(
                    frame,
                    &mut self.state.conversation.get_messages(),
                    &self.state.current_response,
                    self.state.streaming,
                )
            })?;

            if event::poll(Duration::from_millis(25))? {
                let event = event::read()?;
                if let Some(tui_event) = tui.on_event(&event, self.state.streaming) {
                    match tui_event {
                        TUIEvent::Submit(content) => {
                            if content == "/clear" {
                                self.state.clear();
                            } else {
                                self.state.submit_user_message(&content);
                            }
                        }
                        TUIEvent::Exit() => return Ok(()),
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
}
