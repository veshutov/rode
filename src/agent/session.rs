use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::tools::ToolCall;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub role: Role,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_call_id: Option<String>,
    pub used_tokens: Option<u32>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Session {
    system_message: String,
    messages: Vec<Message>,
    max_history: usize,
}

impl Session {
    pub fn new(system_message: String, max_history: usize) -> Self {
        let mut session = Self {
            system_message,
            messages: Vec::new(),
            max_history,
        };
        session.init_system_prompt();
        session
    }

    fn init_system_prompt(&mut self) {
        self.messages.push(Message {
            id: Uuid::now_v7(),
            role: Role::System,
            content: self.system_message.clone(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            used_tokens: None,
            timestamp: Utc::now(),
        });
    }

    pub fn add_user_message(&mut self, content: &str) {
        self.messages.push(Message {
            id: Uuid::now_v7(),
            role: Role::User,
            content: content.to_string(),
            tool_calls: Vec::new(),
            tool_call_id: None,
            used_tokens: None,
            timestamp: Utc::now(),
        });
        self.trim_history();
    }

    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.trim_history();
    }

    pub fn add_tool_message(&mut self, tool_call_id: &str, content: &str) {
        self.messages.push(Message {
            id: Uuid::now_v7(),
            role: Role::Tool,
            content: content.to_string(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.to_string()),
            used_tokens: None,
            timestamp: Utc::now(),
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

    pub fn total_tokens(&self) -> Option<u32> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.used_tokens.is_some())
            .and_then(|m| m.used_tokens)
    }

    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.init_system_prompt();
    }

    /// Serialize the conversation to a JSONL string (one `SessionEntry` per line).
    pub fn to_jsonl(&self) -> Result<String, serde_json::Error> {
        let mut buf = String::new();
        for msg in &self.messages {
            let line = serde_json::to_string(&msg)?;
            buf.push_str(&line);
            buf.push('\n');
        }
        Ok(buf)
    }

    pub fn from_jsonl(
        jsonl: &str,
        default_system_message: &str,
        max_history: usize,
    ) -> Result<Self, serde_json::Error> {
        let mut messages: Vec<Message> = Vec::new();

        for line in jsonl.lines() {
            if line.trim().is_empty() {
                continue;
            }
            let msg: Message = serde_json::from_str(line)?;
            messages.push(msg);
        }

        let system_message = messages
            .iter()
            .find(|m| matches!(m.role, Role::System))
            .map(|m| m.content.clone())
            .unwrap_or_else(|| default_system_message.to_string());

        Ok(Self {
            system_message,
            messages,
            max_history,
        })
    }
}
