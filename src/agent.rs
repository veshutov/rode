use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::message::{Conversation, Message, Role};
use crate::provider::LLMProvider;
use crate::tools::ToolRegistry;

pub enum AgentEvent {
    Token(String),
    MessageDone,
    Finished,
    Error,
}

pub struct Agent {
    pub conversation: Arc<Mutex<Conversation>>,
    tool_registry: ToolRegistry,
    provider: LLMProvider,
    event_tx: UnboundedSender<AgentEvent>,
    cancelled: Arc<AtomicBool>,
}

impl Agent {
    pub fn new(
        conversation: Conversation,
        tool_registry: ToolRegistry,
        provider: LLMProvider,
    ) -> (Self, UnboundedReceiver<AgentEvent>) {
        let (event_tx, event_rx) = unbounded_channel::<AgentEvent>();
        let agent = Self {
            conversation: Arc::new(Mutex::new(conversation)),
            tool_registry,
            provider,
            event_tx,
            cancelled: Arc::new(AtomicBool::new(false)),
        };
        (agent, event_rx)
    }

    pub fn submit_user_message(&mut self, content: &str) {
        {
            let mut conv = self.conversation.lock().unwrap();
            conv.add_message(Role::User, content);
        }
        self.start_stream();
    }

    fn start_stream(&self) {
        self.cancelled.store(false, Ordering::SeqCst);

        let conversation = self.conversation.clone();
        let registry = self.tool_registry.clone();
        let provider = self.provider.clone();
        let tx = self.event_tx.clone();
        let cancelled = self.cancelled.clone();

        tokio::spawn(async move {
            loop {
                if cancelled.load(Ordering::SeqCst) {
                    let _ = tx.send(AgentEvent::Finished);
                    break;
                }

                // Snapshot messages for the API request (short lock)
                let messages: Vec<Message> = {
                    let conv = conversation.lock().unwrap();
                    conv.get_messages().to_vec()
                };

                let result = provider
                    .stream_openai_api(
                        &messages,
                        &registry,
                        |token| {
                            let _ = tx.send(AgentEvent::Token(token.to_string()));
                        },
                        &cancelled,
                    )
                    .await;

                match result {
                    Ok(msg) => {
                        let has_tool_calls = !msg.tool_calls.is_empty();

                        // Add assistant message + execute tools (short lock)
                        {
                            let mut conv = conversation.lock().unwrap();
                            conv.add_assistant_message(&msg.content, msg.tool_calls.clone());

                            for tc in &msg.tool_calls {
                                match registry.execute(tc) {
                                    Ok(output) => {
                                        conv.add_tool_message(&tc.id, &output);
                                    }
                                    Err(e) => {
                                        conv.add_tool_message(&tc.id, &format!("Error: {}", e));
                                    }
                                }
                            }
                        }

                        if has_tool_calls {
                            // Another round — clear the streaming buffer but keep going
                            let _ = tx.send(AgentEvent::MessageDone);
                            continue;
                        }

                        let _ = tx.send(AgentEvent::Finished);
                    }
                    Err(_) => {
                        let _ = tx.send(AgentEvent::Error);
                    }
                }
                break;
            }
        });
    }

    pub fn cancel(&mut self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn clear(&mut self) {
        let mut conv = self.conversation.lock().unwrap();
        conv.clear_messages();
    }
}
