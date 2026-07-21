use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::agent::message::{Conversation, Message};
use crate::agent::provider::LLMProvider;
use crate::tools::ToolRegistry;

pub mod message;
pub mod provider;

pub enum AgentEvent {
    Token(String),
    MessageDone,
    Finished,
    Error(String),
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

    pub fn submit_user_message(&mut self, content: &str, model: &str) {
        {
            let mut conv = self.conversation.lock().unwrap();
            conv.add_user_message(content);
        }
        self.start_stream(model);
    }

    fn start_stream(&self, model: &str) {
        self.cancelled.store(false, Ordering::SeqCst);

        let conversation = self.conversation.clone();
        let registry = self.tool_registry.clone();
        let provider = self.provider.clone();
        let tx = self.event_tx.clone();
        let cancelled = self.cancelled.clone();
        let model = model.to_owned();

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
                        &model,
                        |token| {
                            let _ = tx.send(AgentEvent::Token(token.to_string()));
                        },
                        cancelled.clone(),
                    )
                    .await;

                match result {
                    Ok(msg) => {
                        let has_tool_calls = !msg.tool_calls.is_empty();

                        // Add assistant message (short lock)
                        {
                            let mut conv = conversation.lock().unwrap();
                            conv.add_message(msg.clone());
                        }

                        // Execute tools outside the lock
                        if has_tool_calls {
                            for tc in &msg.tool_calls {
                                match registry.execute(tc).await {
                                    Ok(output) => {
                                        let mut conv = conversation.lock().unwrap();
                                        conv.add_tool_message(&tc.id, &output);
                                    }
                                    Err(e) => {
                                        let mut conv = conversation.lock().unwrap();
                                        conv.add_tool_message(&tc.id, &format!("Error: {}", e));
                                    }
                                }
                            }

                            // Another round — clear the streaming buffer but keep going
                            let _ = tx.send(AgentEvent::MessageDone);
                            continue;
                        }

                        let _ = tx.send(AgentEvent::Finished);
                    }
                    Err(e) => {
                        let _ = tx.send(AgentEvent::Error(e.to_string()));
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
