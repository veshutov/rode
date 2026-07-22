use crossterm::event::EventStream;
use futures::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::{
    agent::{Agent, AgentEvent, provider::LLMProvider, session::Session, store::SessionStore},
    tools::ToolRegistry,
    tui::{TUICommand, Tui, TuiHud},
};

pub struct App {
    tui: Tui,
    agent: Agent,
    event_rx: UnboundedReceiver<AgentEvent>,
    streaming: bool,
    current_response: String,
    status_message: String,
    model: String,
    system_message: String,
    session_store: SessionStore,
}

impl App {
    pub fn new(
        session: Session,
        tool_registry: ToolRegistry,
        provider: LLMProvider,
        model: String,
        system_message: String,
        session_store: SessionStore,
    ) -> Self {
        let (agent, event_rx) = Agent::new(session, tool_registry, provider);
        Self {
            tui: Tui::new(),
            agent,
            event_rx,
            streaming: false,
            current_response: String::new(),
            status_message: String::new(),
            model,
            system_message,
            session_store,
        }
    }

    pub async fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    ) -> anyhow::Result<()> {
        let mut reader = EventStream::new();
        let cwd = std::env::current_dir().unwrap();
        let home = std::env::var("HOME").unwrap();
        let cwd = cwd
            .strip_prefix(&home)
            .map(|p| format!("~/{}", p.display()))
            .unwrap_or_else(|_| cwd.display().to_string());

        loop {
            {
                let conversation = self.agent.session.lock().unwrap();
                let messages = conversation.get_messages();
                let hud = TuiHud {
                    cwd: cwd.clone(),
                    streaming: self.streaming,
                    status_message: self.status_message.clone(),
                    model: self.model.clone(),
                    context_tokens: conversation.total_tokens(),
                };

                terminal.draw(|frame| {
                    self.tui
                        .render(frame, &hud, messages, &self.current_response)
                })?;
            }

            tokio::select! {
                Some(Ok(term_event)) = reader.next() => {
                    if let Some(tui_command) = self.tui.on_event(&term_event, self.streaming) {
                        match tui_command {
                            TUICommand::Submit(content) => {
                                if content.starts_with('/') {
                                    self.handle_slash_command(&content).await;
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

    /// Auto-save the current conversation to the "default" session.
    pub async fn save_default_session(&self) -> anyhow::Result<()> {
        let conv = self.agent.conversation_snapshot();
        self.session_store.save("default", &conv).await?;
        Ok(())
    }

    async fn handle_slash_command(&mut self, input: &str) {
        let parts: Vec<&str> = input.trim().splitn(2, char::is_whitespace).collect();
        let cmd = parts[0];
        let arg = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match cmd {
            "/clear" => {
                self.agent.clear();
                self.tui.reset();
                self.status_message = "Conversation cleared".to_string();
            }
            "/save" => {
                if arg.is_empty() {
                    self.status_message = "Usage: /save <name>".to_string();
                } else {
                    let conv = self.agent.conversation_snapshot();
                    match self.session_store.save(arg, &conv).await {
                        Ok(path) => {
                            self.status_message = format!("Saved session to {}", path.display());
                        }
                        Err(e) => {
                            self.status_message = format!("Save failed: {}", e);
                        }
                    }
                }
            }
            "/load" => {
                if arg.is_empty() {
                    self.status_message = "Usage: /load <name>".to_string();
                } else if self.streaming {
                    self.status_message = "Cannot load while streaming".to_string();
                } else {
                    match self
                        .session_store
                        .load(arg, &self.system_message, usize::MAX)
                        .await
                    {
                        Ok(conv) => {
                            self.agent.replace_conversation(conv);
                            self.tui.reset();
                            self.status_message = format!("Loaded session '{}'", arg);
                        }
                        Err(e) => {
                            self.status_message = format!("Load failed: {}", e);
                        }
                    }
                }
            }
            "/sessions" => match self.session_store.list().await {
                Ok(names) => {
                    if names.is_empty() {
                        self.status_message = "No saved sessions".to_string();
                    } else {
                        self.status_message = format!("Sessions: {}", names.join(", "));
                    }
                }
                Err(e) => {
                    self.status_message = format!("List failed: {}", e);
                }
            },
            _ => {
                self.status_message = format!("Unknown command: {}", input);
            }
        }
    }
}
