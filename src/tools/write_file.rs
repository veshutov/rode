use crate::tools::{Tool, ToolParameter};
use anyhow::Result;
use serde_json::Value;
use tokio::fs;

#[derive(Clone)]
pub struct WriteFileTool;

impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file (overwrite if exists)"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "path".to_string(),
                description: "Path to the file".to_string(),
                r#type: "string".to_string(),
                required: true,
            },
            ToolParameter {
                name: "content".to_string(),
                description: "Content to write".to_string(),
                r#type: "string".to_string(),
                required: true,
            },
        ]
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        let content = args["content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;
        fs::write(path, content).await?;
        Ok(format!("File '{}' written successfully", path))
    }
}
