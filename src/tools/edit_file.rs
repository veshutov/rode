use crate::tools::{Tool, ToolParameter};
use anyhow::Result;
use serde_json::Value;
use tokio::fs;

#[derive(Clone)]
pub struct EditFileTool;

impl Tool for EditFileTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing exact old content with new content"
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
                name: "old_content".to_string(),
                description: "Exact content to replace".to_string(),
                r#type: "string".to_string(),
                required: true,
            },
            ToolParameter {
                name: "new_content".to_string(),
                description: "New content to insert".to_string(),
                r#type: "string".to_string(),
                required: true,
            },
        ]
    }

    async fn execute(&self, args: Value) -> Result<String> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
        let old_content = args["old_content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'old_content' argument"))?;
        let new_content = args["new_content"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'new_content' argument"))?;

        let content = fs::read_to_string(path).await?;
        if !content.contains(old_content) {
            return Ok(format!("Pattern not found in file '{}'", path));
        }

        // Replace only the first occurrence
        let updated = content.replacen(old_content, new_content, 1);
        fs::write(path, updated).await?;
        Ok(format!("File '{}' edited successfully", path))
    }
}
