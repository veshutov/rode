use anyhow::Result;
use std::io::{self, Write};

use crate::message::{Conversation, process_message};
use crate::tools::{
    ToolRegistry, bash::BashTool, edit_file::EditFileTool, read_file::ReadFileTool,
    write_file::WriteFileTool,
};

mod message;
mod provider;
mod render;
mod tools;

#[tokio::main]
async fn main() -> Result<()> {
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(Box::new(BashTool));
    tool_registry.register(Box::new(ReadFileTool));
    tool_registry.register(Box::new(WriteFileTool));
    tool_registry.register(Box::new(EditFileTool));

    let mut conversation = Conversation::new(20);

    loop {
        print!("You: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") {
            break;
        }

        if input.eq_ignore_ascii_case("clear") {
            conversation.clear();
            conversation.add_message(
                "system",
                "You are a helpful AI assistant with access to bash, read_file, write_file, and edit_file tools. Use tools when needed. Respond concisely.",
            );
            println!("Conversation history cleared.");
            continue;
        }

        if !input.is_empty() {
            process_message(input, &mut conversation, &tool_registry).await?;
        }
    }

    Ok(())
}
