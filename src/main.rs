use anyhow::Result;
use crossterm::{
    ExecutableCommand,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{self, Write};

use crate::agent::provider::LLMProvider;
use crate::agent::session::Session;
use crate::agent::store::SessionStore;
use crate::app::App;
use crate::config::AppConfig;
use crate::tools::{
    ToolRegistry, bash::BashTool, edit_file::EditFileTool, read_file::ReadFileTool,
    write_file::WriteFileTool,
};
use std::sync::Arc;

mod agent;
mod app;
mod config;
mod tools;
mod tui;

#[tokio::main]
async fn main() -> Result<()> {
    let home = std::env::var("HOME").unwrap();
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or(format!("{}/.config/rode/config.json", home));

    let config = AppConfig::from_file(&config_path)?;

    let mut tool_registry = ToolRegistry::new();
    tool_registry.register(Arc::new(BashTool));
    tool_registry.register(Arc::new(ReadFileTool));
    tool_registry.register(Arc::new(WriteFileTool));
    tool_registry.register(Arc::new(EditFileTool));

    let provider = LLMProvider::new(config.provider, tool_registry.clone());

    let system_message =
        "You are a coding agent. Project directory = current directory. Respond concisely.";

    let session_store = SessionStore::default_store()?;
    let session = Session::new(system_message.to_owned(), usize::MAX);

    run(
        session,
        tool_registry,
        provider,
        config.model,
        system_message.to_owned(),
        session_store,
    )
    .await?;

    Ok(())
}

pub async fn run(
    session: Session,
    tool_registry: ToolRegistry,
    provider: LLMProvider,
    model: String,
    system_message: String,
    session_store: SessionStore,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    // Enable bracketed paste so multi-line pastes are received as single events
    stdout.write_all(b"\x1b[?2004h")?;
    stdout.flush()?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(
        session,
        tool_registry,
        provider,
        model,
        system_message,
        session_store,
    );
    let result = app.run(&mut terminal).await;

    // Auto-save the session on exit
    if let Err(e) = app.save_default_session().await {
        eprintln!("Warning: auto-save failed: {}", e);
    }

    disable_raw_mode()?;
    let stdout = terminal.backend_mut();
    stdout.write_all(b"\x1b[?2004l")?; // disable bracketed paste
    stdout.execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
