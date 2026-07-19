use anyhow::Result;
use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use dotenv::dotenv;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{self, Write};

use crate::app::App;
use crate::message::Conversation;
use crate::tools::{
    ToolRegistry, bash::BashTool, edit_file::EditFileTool, read_file::ReadFileTool,
    write_file::WriteFileTool,
};

mod app;
mod message;
mod provider;
mod state;
mod tools;
mod tui;

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

    run(conversation, tool_registry).await?;

    Ok(())
}

pub async fn run(conversation: Conversation, tool_registry: ToolRegistry) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    // Enable bracketed paste so multi-line pastes are received as single events
    stdout.write_all(b"\x1b[?2004h")?;
    stdout.flush()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (mut app, event_rx) = App::new(conversation, tool_registry);
    let result = app.run(&mut terminal, &event_rx);

    disable_raw_mode()?;
    let stdout = terminal.backend_mut();
    stdout.write_all(b"\x1b[?2004l")?; // disable bracketed paste
    stdout.execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
