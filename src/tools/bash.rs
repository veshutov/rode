use crate::tools::{Tool, ToolParameter};
use anyhow::Result;
use serde_json::Value;
use std::process::Command;

pub struct BashTool;

impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![ToolParameter {
            name: "command".to_string(),
            description: "Bash command to execute".to_string(),
            r#type: "string".to_string(),
            required: true,
        }]
    }

    fn execute(&self, args: Value) -> Result<String> {
        let command = args["command"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'command' argument"))?;

        let dangerous_patterns = [
            "rm -rf",
            "rm -fr",
            ":(){ :|:& };:",
            "mkfs.",
            "dd if=",
            "> /dev/sda",
            "mv / ",
            "mv /* ",
            "chmod -R 000 /",
        ];

        let lower_cmd = command.to_lowercase();
        for pattern in &dangerous_patterns {
            if lower_cmd.contains(pattern) {
                return Ok(format!(
                    "Error: Command blocked for security reasons (pattern: {})",
                    pattern
                ));
            }
        }

        let output = Command::new("sh").arg("-c").arg(command).output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            Ok(stdout.to_string())
        } else {
            Ok(format!(
                "Error (exit code: {:?}):\n{}",
                output.status.code(),
                stderr
            ))
        }
    }
}
