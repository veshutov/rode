use anyhow::Result;
use crossterm::{ExecutableCommand, cursor, terminal};
use std::io::{self, Write};

use crate::message::{Conversation, Message};
use crate::{
    provider::stream_openai_api,
    render::print_markdown,
    tools::{ToolCall, ToolRegistry},
};

pub async fn run(mut conversation: Conversation, tool_registry: ToolRegistry) -> Result<()> {
    println!("Rode CLI - Type your message or 'exit' to quit, 'clear' to reset history");

    loop {
        print!("[You]: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") {
            break;
        }

        if input.eq_ignore_ascii_case("clear") {
            conversation.reset();
            println!("Conversation history cleared.");
            continue;
        }

        if !input.is_empty() {
            if let Err(e) = handle_streaming_turn(input, &mut conversation, &tool_registry).await {
                eprintln!("Error: {}", e);
            }
        }
    }

    Ok(())
}

pub async fn handle_streaming_turn(
    input: &str,
    conversation: &mut Conversation,
    tool_registry: &ToolRegistry,
) -> Result<()> {
    // Stream the assistant response
    let mut assistant_content = String::new();
    print!("[Assistant]: ");
    io::stdout().flush()?;

    // Save cursor position right after the prefix so we can restore here later
    let mut stdout = io::stdout();
    stdout.execute(cursor::SavePosition)?;

    let mut response = process_message_streaming(input, conversation, tool_registry, |token| {
        assistant_content.push_str(token);
        if !token.trim().is_empty() {
            print!("{}", token);
            io::stdout().flush().unwrap();
        }
    })
    .await?;

    // If no tool calls, restore to saved position, clear everything below,
    // and render the final markdown nicely
    if response.tool_calls.is_empty() {
        stdout.execute(cursor::RestorePosition)?;
        stdout.execute(terminal::Clear(terminal::ClearType::FromCursorDown))?;
        print_markdown(&assistant_content);
    } else {
        println!();
    }

    // Handle tool calls if any
    loop {
        if response.tool_calls.is_empty() {
            break;
        }

        execute_tool_calls(
            &response.tool_calls,
            conversation,
            tool_registry,
            |name, args| {
                println!(
                    "[Tool {}]: {}",
                    name,
                    args.trim().chars().take(50).collect::<String>()
                );
            },
        );

        // Stream follow-up after tool execution
        assistant_content.clear();
        print!("[Assistant]: ");
        io::stdout().flush()?;
        stdout.execute(cursor::SavePosition)?;

        response = stream_assistant_response(conversation, tool_registry, |token| {
            assistant_content.push_str(token);
            if !token.trim().is_empty() {
                print!("{}", token);
                io::stdout().flush().unwrap();
            }
        })
        .await?;

        if response.tool_calls.is_empty() {
            stdout.execute(cursor::RestorePosition)?;
            stdout.execute(terminal::Clear(terminal::ClearType::FromCursorDown))?;
            print_markdown(&assistant_content);
        } else {
            println!();
        }
    }

    Ok(())
}

/// Process a message with streaming support. The on_token callback is called for each token.
/// Returns the final assistant message (including any tool calls).
pub async fn process_message_streaming(
    message: &str,
    conversation: &mut Conversation,
    tool_registry: &ToolRegistry,
    mut on_token: impl FnMut(&str),
) -> Result<Message> {
    conversation.add_message("user", message);

    let response = stream_openai_api(conversation, tool_registry, |token| {
        on_token(token);
    })
    .await?;

    // Add the assistant message to conversation
    conversation.add_assistant_message(&response.content, response.tool_calls.clone());

    Ok(response)
}

/// Stream an assistant response without adding a user message.
/// Use this for follow-up after tool execution.
pub async fn stream_assistant_response(
    conversation: &mut Conversation,
    tool_registry: &ToolRegistry,
    mut on_token: impl FnMut(&str),
) -> Result<Message> {
    let response = stream_openai_api(conversation, tool_registry, |token| {
        on_token(token);
    })
    .await?;

    // Add the assistant message to conversation
    conversation.add_assistant_message(&response.content, response.tool_calls.clone());

    Ok(response)
}

/// Execute tool calls and add results to conversation. Returns true if any tools were executed.
pub fn execute_tool_calls(
    tool_calls: &[ToolCall],
    conversation: &mut Conversation,
    tool_registry: &ToolRegistry,
    mut on_tool_start: impl FnMut(&str, &str),
) -> bool {
    if tool_calls.is_empty() {
        return false;
    }

    for tool_call in tool_calls {
        on_tool_start(&tool_call.name, &tool_call.arguments);
        let result = tool_registry
            .execute(tool_call)
            .unwrap_or_else(|e| format!("Error: {}", e));
        conversation.add_tool_message(&tool_call.id, &result);
    }

    true
}
