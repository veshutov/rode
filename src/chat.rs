use crate::{provider::call_openai_api, tool::ToolCall};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Conversation {
    messages: Vec<Message>,
    max_history: usize,
}

impl Conversation {
    pub fn new(max_history: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_history,
        }
    }

    pub fn add_message(&mut self, role: &str, content: &str) {
        self.messages.push(Message {
            role: role.to_string(),
            content: content.to_string(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        });
        self.trim_history();
    }

    pub fn add_assistant_message(&mut self, content: &str, tool_calls: Vec<ToolCall>) {
        self.messages.push(Message {
            role: "assistant".to_string(),
            content: content.to_string(),
            tool_calls,
            tool_call_id: None,
        });
        self.trim_history();
    }

    pub fn add_tool_message(&mut self, tool_call_id: &str, content: &str) {
        self.messages.push(Message {
            role: "tool".to_string(),
            content: content.to_string(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.to_string()),
        });
        self.trim_history();
    }

    fn trim_history(&mut self) {
        if self.messages.len() > self.max_history {
            if self.messages.len() > 2 {
                let drain_end = self.messages.len() - self.max_history + 1;
                self.messages.drain(1..drain_end);
            }
        }
    }

    pub fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

pub async fn process_message(message: &str, conversation: &mut Conversation) -> Result<()> {
    conversation.add_message("user", message);

    loop {
        match call_openai_api(conversation).await {
            Ok(response) => {
                if !response.tool_calls.is_empty() {
                    conversation
                        .add_assistant_message(&response.content, response.tool_calls.clone());

                    for tool_call in &response.tool_calls {
                        let result = tool_call
                            .execute()
                            .unwrap_or_else(|e| format!("Error: {}", e));
                        println!("[Tool {}]: {}", tool_call.name, tool_call.arguments);
                        conversation.add_tool_message(&tool_call.id, &result);
                    }
                    // Loop back to API so the model can see the results
                } else {
                    println!("[Assistant]: {}", response.content);
                    conversation.add_message("assistant", &response.content);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error calling OpenAI API: {}", e);
                println!("[Assistant]: Sorry, I encountered an error processing your request.");
                break;
            }
        }
    }

    Ok(())
}
