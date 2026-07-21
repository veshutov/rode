use crate::tools::{Tool, ToolParameter};
use anyhow::Result;
use serde_json::Value;
use tokio::fs;

#[derive(Clone)]
pub struct ReadFileTool;

impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the full contents of a file"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter {
            name: "path".to_string(),
            description: "Path to the file".to_string(),
            r#type: "string".to_string(),
            required: true,
        }]
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        Ok(fs::read_to_string(path).await?)
    }
}
