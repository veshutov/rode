use anyhow::Result;
use crossterm::{ExecutableCommand, cursor, terminal};
use std::io::{self, Write};

use crate::message::{Conversation, Message, Role};
use crate::{
    provider::stream_openai_api,
    render::print_markdown,
    tools::{ToolCall, ToolRegistry},
};

const USER_COLOR: &str = "\x1b[38;2;163;217;170m"; // Mint Green (#A3D9AA)
const AGENT_COLOR: &str = "\x1b[38;2;187;154;247m"; // Lavender (#BB9AF7)
const TOOL_COLOR: &str = "\x1b[38;2;255;158;100m"; // Peach/Orange (#FF9E64)

const RESET: &str = "\x1b[0m";

pub async fn run(mut conversation: Conversation, tool_registry: ToolRegistry) -> Result<()> {
    println!("Rode CLI - Type your message or 'exit' to quit, 'clear' to reset history");

    loop {
        print!("{}[You]: {}{}", USER_COLOR, RESET, " ");
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
    let mut response = stream_response(Some(input), conversation, tool_registry).await?;

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
                    "{}[Tool {}]: {}{}",
                    TOOL_COLOR,
                    name,
                    args.trim().chars().take(50).collect::<String>(),
                    RESET
                );
            },
        );

        response = stream_response(None, conversation, tool_registry).await?;
    }

    Ok(())
}

/// Stream a response from the assistant.
/// If `user_message` is Some, it will be added to the conversation before streaming.
/// If None, it streams a follow-up response (e.g. after tool execution).
async fn stream_response(
    user_message: Option<&str>,
    conversation: &mut Conversation,
    tool_registry: &ToolRegistry,
) -> Result<Message> {
    if let Some(msg) = user_message {
        conversation.add_message(Role::User, msg);
    }

    let mut assistant_content = String::new();
    print!("{}[Assistant]: {}{}", AGENT_COLOR, RESET, " ");
    io::stdout().flush()?;

    let mut stdout = io::stdout();
    stdout.execute(cursor::SavePosition)?;

    let response = stream_openai_api(conversation, tool_registry, |token| {
        assistant_content.push_str(token);
        if !token.trim().is_empty() {
            print!("{}", token);
            io::stdout().flush().unwrap();
        }
    })
    .await?;

    conversation.add_assistant_message(&response.content, response.tool_calls.clone());

    if response.tool_calls.is_empty() {
        stdout.execute(cursor::RestorePosition)?;
        stdout.execute(terminal::Clear(terminal::ClearType::FromCursorDown))?;
        print_markdown(&assistant_content);
    } else {
        println!();
    }

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
