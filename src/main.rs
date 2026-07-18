use crate::chat::{Conversation, process_message};
use anyhow::Result;
use std::io::{self, Write};

mod chat;
mod provider;
mod tool;

#[tokio::main]
async fn main() -> Result<()> {
    let mut conversation = Conversation::new(20);
    conversation.add_message(
        "system",
        "You are a helpful AI assistant with access to bash, read_file, write_file, and edit_file tools. Use tools when needed. Respond concisely.",
    );

    println!("Minimal CLI Agent - Interactive Mode");
    println!("Type 'exit' to quit, 'clear' to reset conversation\n");

    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") {
            println!("Goodbye!");
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
            process_message(input, &mut conversation).await?;
        }
    }

    Ok(())
}
