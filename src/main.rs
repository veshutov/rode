use anyhow::Result;
use dotenv::dotenv;

use crate::message::Conversation;
use crate::tools::{
    ToolRegistry, bash::BashTool, edit_file::EditFileTool, read_file::ReadFileTool,
    write_file::WriteFileTool,
};

mod app;
mod message;
mod provider;
mod render;
mod repl;
mod tools;

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(std::sync::Arc::new(BashTool));
    tool_registry.register(std::sync::Arc::new(ReadFileTool));
    tool_registry.register(std::sync::Arc::new(WriteFileTool));
    tool_registry.register(std::sync::Arc::new(EditFileTool));

    let system_message =
        "You are a coding agent. Project directory = current directory. Respond concisely.";
    let mut conversation = Conversation::new(system_message.to_string(), 20);
    conversation.init();

    repl::run(conversation, tool_registry).await?;

    Ok(())
}
