use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub mod bash;
pub mod edit_file;
pub mod read_file;
pub mod write_file;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Vec<ToolParameter>;
    fn execute(&self, args: Value) -> Result<String>;
}

#[derive(Debug, Clone)]
pub struct ToolParameter {
    pub r#type: String,
    pub name: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

#[derive(Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        self.tools.insert(name, tool);
    }

    pub fn execute(&self, tool_call: &ToolCall) -> Result<String> {
        match self.tools.get(&tool_call.name) {
            Some(tool) => {
                let args: Value = serde_json::from_str(&tool_call.arguments)?;
                tool.execute(args)
            }
            None => Err(anyhow::anyhow!("Tool '{}' not found", tool_call.name)),
        }
    }

    pub fn get_available_tools(&self) -> Vec<&Arc<dyn Tool>> {
        self.tools.values().collect()
    }
}
