use anyhow::Result;

use crate::{
    provider::call_openai_api,
    render::print_markdown,
    tools::{ToolCall, ToolRegistry},
};

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
        if self.messages.len() <= self.max_history {
            return;
        }

        // Calculate how many to remove, keeping system message at index 0
        let excess = self.messages.len() - self.max_history;
        let drain_end = 1 + excess;

        if drain_end > 1 && drain_end <= self.messages.len() {
            self.messages.drain(1..drain_end);
        }
    }

    pub fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

pub async fn process_message(
    message: &str,
    conversation: &mut Conversation,
    tool_registry: &ToolRegistry,
) -> Result<()> {
    conversation.add_message("user", message);

    loop {
        match call_openai_api(conversation, tool_registry).await {
            Ok(response) => {
                if !response.tool_calls.is_empty() {
                    conversation
                        .add_assistant_message(&response.content, response.tool_calls.clone());

                    for tool_call in &response.tool_calls {
                        println!(
                            "Executing tool: {} with args: {}",
                            tool_call.name, tool_call.arguments
                        );
                        let result = tool_registry
                            .execute(tool_call)
                            .unwrap_or_else(|e| format!("Error: {}", e));
                        println!(
                            "[Tool {}]: {}\n[Result]: {}",
                            tool_call.name, tool_call.arguments, result
                        );
                        conversation.add_tool_message(&tool_call.id, &result);
                    }
                    // Loop back to API so the model can see the results
                } else {
                    println!("\n[Assistant]:");
                    print_markdown(&response.content);
                    conversation.add_message("assistant", &response.content);
                    break;
                }
            }
            Err(e) => {
                eprintln!("Error calling API: {}", e);
                println!("[Assistant]: Sorry, I encountered an error processing your request.");
                break;
            }
        }
    }

    Ok(())
}
