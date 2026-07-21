use crossterm::event::EventStream;
use futures::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
    agent::message::Conversation,
    agent::provider::LLMProvider,
    agent::{Agent, AgentEvent},
    tools::ToolRegistry,
    tui::{TUICommand, Tui},
};

pub struct App {
    tui: Tui,
    agent: Agent,
    event_rx: UnboundedReceiver<AgentEvent>,
    streaming: bool,
    current_response: String,
    status_message: String,
    model: String,
}

impl App {
    pub fn new(
        conversation: Conversation,
        tool_registry: ToolRegistry,
        provider: LLMProvider,
        model: String,
    ) -> Self {
        let (agent, event_rx) = Agent::new(conversation, tool_registry, provider);
        Self {
            tui: Tui::new(),
            agent,
            event_rx,
            streaming: false,
            current_response: String::new(),
            status_message: String::new(),
            model,
        }
    }

    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<()> {
        let mut reader = EventStream::new();

        loop {
            {
                let conversation = self.agent.conversation.lock().unwrap();
                let messages = conversation.get_messages();
                let context_tokens = conversation.total_tokens();

                terminal.draw(|frame| {
                    self.tui.render(
                        frame,
                        messages,
                        &self.current_response,
                        self.streaming,
                        &self.status_message,
                        context_tokens,
                    )
                })?;
            }

            tokio::select! {
                Some(Ok(term_event)) = reader.next() => {
                    if let Some(tui_command) = self.tui.on_event(&term_event, self.streaming) {
                        match tui_command {
                            TUICommand::Submit(content) => {
                                if content.starts_with('/') {
                                    self.handle_slash_command(&content);
                                } else {
                                    self.status_message.clear();
                                    self.streaming = true;
                                    self.agent.submit_user_message(&content, &self.model);
                                }
                            }
                            TUICommand::Exit => return Ok(()),
                            TUICommand::Cancel => {
                                if self.streaming {
                                    self.agent.cancel();
                                    self.current_response.clear();
                                    self.streaming = false;
                                }
                            }
                        }
                    }
                }
                Some(agent_event) = self.event_rx.recv() => {
                    match agent_event {
                        AgentEvent::Token(token) => {
                            self.current_response.push_str(&token);
                        }
                        AgentEvent::MessageDone => {
                            self.current_response.clear();
                        }
                        AgentEvent::Finished => {
                            self.current_response.clear();
                            self.streaming = false;
                        }
                        AgentEvent::Error(e) => {
                            self.current_response.clear();
                            self.streaming = false;
                            self.status_message = e;
                        }
                    }
                }
            }
        }
    }

    fn handle_slash_command(&mut self, input: &str) {
        match input {
            "/clear" => {
                self.agent.clear();
                self.tui.reset();
                self.status_message = "Conversation cleared".to_string();
            }
            _ => {
                self.status_message = format!("Unknown command: /{}", input);
                self.tui.reset();
            }
        }
    }
}
