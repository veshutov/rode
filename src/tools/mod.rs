use anyhow::Result;
use serde_json::Value;

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

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    // pub fn execute(&self, name: &str, args: Value) -> Result<String> {
    //     match self.tools.iter().find(|t| t.name() == name) {
    //         Some(tool) => tool.execute(args),
    //         None => Err(anyhow::anyhow!("Tool '{}' not found", name)),
    //     }
    // }

    pub fn execute(&self, tool_call: &ToolCall) -> Result<String> {
        match self.tools.iter().find(|t| t.name() == tool_call.name) {
            Some(tool) => {
                let args: Value = serde_json::from_str(&tool_call.arguments)?;
                tool.execute(args)
            }
            None => Err(anyhow::anyhow!("Tool '{}' not found", tool_call.name)),
        }
    }

    pub fn get_available_tools(&self) -> &Vec<Box<dyn Tool>> {
        &self.tools
    }
}
