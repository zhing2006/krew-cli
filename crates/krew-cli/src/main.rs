mod app;
mod render;

use clap::Parser;

/// krew CLI argument definitions.
#[derive(Parser, Debug)]
#[command(name = "krew", version, about = "Multi-Agent Meeting CLI")]
struct Cli {
    /// Path to config file.
    #[arg(short, long, value_name = "PATH")]
    config: Option<String>,

    /// Agents to enable (comma-separated, overrides config).
    #[arg(short, long, value_name = "NAMES")]
    agents: Option<String>,

    /// Tool approval mode (suggest, auto-edit, full-auto).
    #[arg(long, value_name = "MODE")]
    approval_mode: Option<String>,

    /// Resume a session (optionally by ID).
    #[arg(long, value_name = "ID")]
    resume: Option<Option<String>>,

    /// Enable verbose output.
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();

    let cwd = std::env::current_dir()?;
    let app = app::App::new(cwd)?;

    // TEMP: should remove.
    if app.project_instructions.is_some() {
        tracing::info!(cwd = %app.cwd.display(), "Loaded project instructions from AGENTS.md");
    }

    Ok(())
}
