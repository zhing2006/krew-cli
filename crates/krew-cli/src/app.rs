//! App state machine and main event loop.

use std::path::PathBuf;

/// Top-level application state.
pub struct App {
    /// Current working directory for the session.
    pub cwd: PathBuf,
    /// Project-level instructions loaded from AGENTS.md files (if any).
    pub project_instructions: Option<String>,
}

impl App {
    /// Initialize the application, loading config and project instructions.
    pub fn new(cwd: PathBuf) -> anyhow::Result<Self> {
        let project_instructions = match krew_config::load_project_instructions(&cwd) {
            Ok(instructions) => instructions,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load project instructions");
                None
            }
        };

        Ok(Self {
            cwd,
            project_instructions,
        })
    }
}
