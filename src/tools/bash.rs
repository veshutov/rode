use crate::tools::{Tool, ToolParameter};
use anyhow::Result;
use serde_json::Value;
use std::time::Duration;
use tokio::process::Command;

const DEFAULT_TIMEOUT_SECS: u64 = 30;

#[derive(Clone)]
pub struct BashTool;

impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command"
    }

    fn parameters(&self) -> Vec<ToolParameter> {
        vec![
            ToolParameter {
                name: "command".to_string(),
                description: "Bash command to execute".to_string(),
                r#type: "string".to_string(),
                required: true,
            },
            ToolParameter {
                name: "timeout".to_string(),
                description: "Timeout in seconds (default 30)".to_string(),
                r#type: "integer".to_string(),
                required: false,
            },
        ]
    }

    async fn execute(&self, args: Value) -> Result<String> {
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

        let timeout = args["timeout"].as_u64().unwrap_or(DEFAULT_TIMEOUT_SECS);

        let child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stderr(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let output = match tokio::time::timeout(
            Duration::from_secs(timeout),
            child.wait_with_output(),
        )
        .await
        {
            Ok(result) => result?,
            Err(_) => {
                return Ok(format!(
                    "Error: Command timed out after {} seconds",
                    timeout
                ));
            }
        };

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(stdout)
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            Ok(format!(
                "Error (exit code: {:?}):\n{}",
                output.status.code(),
                stderr
            ))
        }
    }
}
