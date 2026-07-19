use crate::message::{Conversation, Message, Role};
use crate::provider;
use crate::tools::ToolRegistry;
use crate::tui::TUI;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

#[derive(Clone)]
pub enum LLMEvent {
    Token(String),
    Done(Message),
    Error(String),
}

pub struct AppState {
    pub tui: TUI,
    pub conversation: Conversation,
    pub tool_registry: ToolRegistry,
    pub streaming: bool,
    pub current_response: String,
    event_tx: Sender<LLMEvent>,
    pub cancelled: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(
        conversation: Conversation,
        tool_registry: ToolRegistry,
    ) -> (Self, Receiver<LLMEvent>) {
        let (event_tx, event_rx) = channel::<LLMEvent>();
        let state = Self {
            tui: TUI::new(),
            conversation,
            tool_registry,
            streaming: false,
            current_response: String::new(),
            event_tx,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        (state, event_rx)
    }

    pub fn submit_user_message(&mut self, content: &str) {
        self.conversation.add_message(Role::User, content);
        self.tui.scroll.set_auto(true);
        self.start_stream();
    }

    pub fn start_stream(&mut self) {
        self.streaming = true;
        self.tui.scroll.set_auto(true);
        self.current_response.clear();
        self.cancelled.store(false, Ordering::SeqCst);
        let conv = self.conversation.clone();
        let registry = self.tool_registry.clone();
        let tx = self.event_tx.clone();

        let cancelled = self.cancelled.clone();
        tokio::spawn(async move {
            let result = provider::stream_openai_api(
                &conv,
                &registry,
                |token| {
                    let _ = tx.send(LLMEvent::Token(token.to_string()));
                },
                cancelled,
            )
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

    pub fn handle_llm_event(&mut self, event: LLMEvent) -> Vec<Message> {
        match event {
            LLMEvent::Token(token) => {
                self.current_response.push_str(&token);
                Vec::new()
            }
            LLMEvent::Done(msg) => {
                self.streaming = false;
                self.conversation
                    .add_assistant_message(&msg.content, msg.tool_calls.clone());
                self.current_response.clear();

                let mut followups = Vec::new();
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
                    followups.push(msg);
                }
                followups
            }
            LLMEvent::Error(e) => {
                self.streaming = false;
                self.current_response.clear();
                self.conversation
                    .add_message(Role::Assistant, &format!("Error: {}", e));
                Vec::new()
            }
        }
    }

    pub fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.streaming = false;
        self.current_response.clear();
    }

    pub fn clear(&mut self) {
        self.conversation.clear_messages();
        self.tui.reset();
        self.streaming = false;
        self.current_response.clear();
    }
}
