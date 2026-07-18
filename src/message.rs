use crate::tools::ToolCall;

#[derive(Debug, Clone)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Conversation {
    system_message: String,
    messages: Vec<Message>,
    max_history: usize,
}

impl Conversation {
    pub fn new(system_message: String, max_history: usize) -> Self {
        Self {
            system_message,
            messages: Vec::new(),
            max_history,
        }
    }

    pub fn init(&mut self) {
        self.init_system_prompt();
    }

    fn init_system_prompt(&mut self) {
        self.messages.push(Message {
            role: "system".to_string(),
            content: self.system_message.clone(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        });
        self.trim_history();
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

    pub fn reset(&mut self) {
        self.messages.clear();
        self.init_system_prompt();
    }
}
