use std::path::{Path, PathBuf};
use tokio::fs;

use anyhow::{Context, Result};

use crate::agent::session::Session;

/// Manages JSONL session files stored in the config folder.
pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    /// Create a `SessionStore` rooted at `dir` (e.g. `~/.config/rode/sessions`).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Default store rooted at `~/.config/rode/sessions`.
    pub fn default_store() -> Result<Self> {
        let home = std::env::var("HOME").context("HOME is not set")?;
        Ok(Self::new(format!("{}/.config/rode/sessions", home)))
    }

    /// Ensure the sessions directory exists.
    pub async fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .await
            .with_context(|| format!("Failed to create sessions dir: {}", self.dir.display()))
    }

    /// Path to the JSONL file for `name`.
    pub fn path_for(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{}.jsonl", name))
    }

    /// Save a conversation to `<name>.jsonl`.
    pub async fn save(&self, name: &str, session: &Session) -> Result<PathBuf> {
        self.ensure_dir().await?;
        let path = self.path_for(name);
        let jsonl = session
            .to_jsonl()
            .context("Failed to serialize conversation")?;
        fs::write(&path, jsonl)
            .await
            .with_context(|| format!("Failed to write session file: {}", path.display()))?;
        Ok(path)
    }

    /// Load a conversation from `<name>.jsonl`.
    pub async fn load(
        &self,
        name: &str,
        default_system_message: &str,
        max_history: usize,
    ) -> Result<Session> {
        let path = self.path_for(name);
        let contents = fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read session file: {}", path.display()))?;
        Session::from_jsonl(&contents, default_system_message, max_history)
            .with_context(|| format!("Failed to parse session file: {}", path.display()))
    }

    /// List all saved session names (without the `.jsonl` extension), sorted
    /// by last-modified time (newest first).
    pub async fn list(&self) -> Result<Vec<String>> {
        if !self.dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries = fs::read_dir(&self.dir)
            .await
            .with_context(|| format!("Failed to read sessions dir: {}", self.dir.display()))?;

        let mut results = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("jsonl") {
                continue;
            }
            let stem = path
                .file_stem()
                .ok_or(anyhow::anyhow!("invalid session file"))?
                .to_string_lossy()
                .to_string();
            let mtime = entry.metadata().await?.modified()?;
            results.push((stem, mtime));
        }
        results.sort_by_key(|a| a.1);
        Ok(results.into_iter().rev().map(|(name, _)| name).collect())
    }

    /// Delete `<name>.jsonl`. Returns `true` if a file was removed.
    pub async fn delete(&self, name: &str) -> Result<bool> {
        let path = self.path_for(name);
        if path.exists() {
            fs::remove_file(&path)
                .await
                .with_context(|| format!("Failed to delete session file: {}", path.display()))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Return the directory path (for display).
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}
