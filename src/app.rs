use crossterm::event::EventStream;
use futures::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
    agent::{Agent, AgentEvent},
    message::Conversation,
    provider::{LLMProvider, LLMProviderConfig},
    tools::ToolRegistry,
    tui::{TUI, TUICommand},
};

pub struct App {
    tui: TUI,
    agent: Agent,
    event_rx: UnboundedReceiver<AgentEvent>,
    streaming: bool,
    current_response: String,
}

impl App {
    pub fn new(conversation: Conversation, tool_registry: ToolRegistry) -> Self {
        let provider = LLMProvider::new(LLMProviderConfig::default());
        let (agent, event_rx) = Agent::new(conversation, tool_registry, provider);
        Self {
            tui: TUI::new(),
            agent,
            event_rx,
            streaming: false,
            current_response: String::new(),
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

                terminal.draw(|frame| {
                    self.tui
                        .render(frame, &messages, &self.current_response, self.streaming)
                })?;
            }

            tokio::select! {
                Some(Ok(term_event)) = reader.next() => {
                    if let Some(tui_command) = self.tui.on_event(&term_event, self.streaming) {
                        match tui_command {
                            TUICommand::Submit(content) => {
                                if content == "/clear" {
                                    self.agent.clear();
                                    self.tui.reset();
                                } else {
                                    self.streaming = true;
                                    self.agent.submit_user_message(&content);
                                }
                            }
                            TUICommand::Exit => return Ok(()),
                            TUICommand::Cancel => {
                                if self.streaming {
                                    self.agent.cancel();
                                    self.streaming = false;
                                    self.current_response.clear();
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
                            self.streaming = false;
                            self.current_response.clear();
                        }
                        AgentEvent::Error => {
                            self.streaming = false;
                            self.current_response.clear();
                        }
                    }
                }
            }
        }
    }
}
