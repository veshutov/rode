use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub mod bash;
pub mod edit_file;
pub mod read_file;
pub mod write_file;

use bash::BashTool;
use edit_file::EditFileTool;
use read_file::ReadFileTool;
use write_file::WriteFileTool;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Vec<ToolParameter>;
    async fn execute(&self, args: Value) -> Result<String>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameter {
    pub r#type: String,
    pub name: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Clone)]
pub enum ToolKind {
    Bash(BashTool),
    ReadFile(ReadFileTool),
    WriteFile(WriteFileTool),
    EditFile(EditFileTool),
}

impl Tool for ToolKind {
    fn name(&self) -> &str {
        match self {
            ToolKind::Bash(t) => t.name(),
            ToolKind::ReadFile(t) => t.name(),
            ToolKind::WriteFile(t) => t.name(),
            ToolKind::EditFile(t) => t.name(),
        }
    }

    fn description(&self) -> &str {
        match self {
            ToolKind::Bash(t) => t.description(),
            ToolKind::ReadFile(t) => t.description(),
            ToolKind::WriteFile(t) => t.description(),
            ToolKind::EditFile(t) => t.description(),
        }
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        match self {
            ToolKind::Bash(t) => t.parameters(),
            ToolKind::ReadFile(t) => t.parameters(),
            ToolKind::WriteFile(t) => t.parameters(),
            ToolKind::EditFile(t) => t.parameters(),
        }
    }

    async fn execute(&self, args: Value) -> Result<String> {
        match self {
            ToolKind::Bash(t) => t.execute(args).await,
            ToolKind::ReadFile(t) => t.execute(args).await,
            ToolKind::WriteFile(t) => t.execute(args).await,
            ToolKind::EditFile(t) => t.execute(args).await,
        }
    }
}

#[derive(Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, ToolKind>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: ToolKind) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub async fn execute(&self, tool_call: &ToolCall) -> Result<String> {
        match self.tools.get(&tool_call.name) {
            Some(tool) => {
                let args: Value = serde_json::from_str(&tool_call.arguments)?;
                tool.execute(args).await
            }
            None => Err(anyhow::anyhow!("Tool '{}' not found", tool_call.name)),
        }
    }

    pub fn available_tools(&self) -> Vec<ToolInfo> {
        self.tools
            .values()
            .map(|tool| ToolInfo {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters(),
            })
            .collect()
    }
}
