use anyhow::Result;
use serde_json::Value;
use std::fs;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ToolParameter>,
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

impl ToolCall {
    pub fn execute(&self) -> Result<String> {
        let args: Value = serde_json::from_str(&self.arguments)?;

        match self.name.as_str() {
            "read_file" => {
                let path = args["path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
                Ok(fs::read_to_string(path)?)
            }
            "write_file" => {
                let path = args["path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
                let content = args["content"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'content' argument"))?;
                fs::write(path, content)?;
                Ok("File written successfully".to_string())
            }
            "edit_file" => {
                let path = args["path"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'path' argument"))?;
                let old_content = args["old_content"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'old_content' argument"))?;
                let new_content = args["new_content"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'new_content' argument"))?;

                let content = fs::read_to_string(path)?;
                if !content.contains(old_content) {
                    return Ok(format!("Pattern '{}' not found in file", old_content));
                }
                let updated = content.replace(old_content, new_content);
                fs::write(path, updated)?;
                Ok("File edited successfully".to_string())
            }
            "bash" => {
                let command = args["command"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;

                let dangerous = ["rm -rf", "rm -fr", ":(){ :|:& };:"];
                for d in &dangerous {
                    if command.contains(d) {
                        return Ok("Error: Command blocked for security reasons".to_string());
                    }
                }

                let output = Command::new("sh").arg("-c").arg(command).output()?;
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).to_string())
                } else {
                    Ok(format!(
                        "Error: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ))
                }
            }
            _ => Ok(format!("Unknown tool: {}", self.name)),
        }
    }
}

pub fn get_available_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "bash".to_string(),
            description: "Execute a bash command".to_string(),
            parameters: vec![ToolParameter {
                name: "command".to_string(),
                description: "Bash command to execute".to_string(),
                r#type: "string".to_string(),
                required: true,
            }],
        },
        Tool {
            name: "read_file".to_string(),
            description: "Read the full contents of a file".to_string(),
            parameters: vec![ToolParameter {
                name: "path".to_string(),
                description: "Path to the file".to_string(),
                r#type: "string".to_string(),
                required: true,
            }],
        },
        Tool {
            name: "write_file".to_string(),
            description: "Write content to a file (overwrite if exists)".to_string(),
            parameters: vec![
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
            ],
        },
        Tool {
            name: "edit_file".to_string(),
            description: "Edit a file by replacing exact old content with new content".to_string(),
            parameters: vec![
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
            ],
        },
    ]
}
