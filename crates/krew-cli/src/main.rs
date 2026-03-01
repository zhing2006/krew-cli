mod app;
mod completion;
mod custom_terminal;
mod frame_scheduler;
mod render;
mod streaming;
mod textarea;

use std::io::{self, stdout};
use std::path::{Path, PathBuf};

use clap::Parser;
use crossterm::event::{
    DisableBracketedPaste, EnableBracketedPaste, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use krew_config::Config;
use ratatui::crossterm::execute;

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

/// Default number of days to retain log files.
const LOG_RETENTION_DAYS: u64 = 7;

/// Initialize tracing logging with daily-rolling file output.
///
/// Returns the `WorkerGuard` that MUST be held alive for the program's
/// lifetime; dropping it flushes remaining buffered logs.
fn init_logging(
    cwd: &Path,
    verbose: bool,
) -> anyhow::Result<tracing_appender::non_blocking::WorkerGuard> {
    let log_dir = cwd.join(".krew").join("logs");
    std::fs::create_dir_all(&log_dir)?;

    // Clean up old log files beyond the retention period.
    clean_old_logs(&log_dir, LOG_RETENTION_DAYS);

    let file_appender = tracing_appender::rolling::daily(&log_dir, "krew.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let level = if verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_max_level(level)
        .init();

    Ok(guard)
}

/// Delete log files older than `retention_days` from the given directory.
fn clean_old_logs(log_dir: &Path, retention_days: u64) {
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(retention_days * 24 * 60 * 60);

    let entries = match std::fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Warning: failed to read log directory: {e}");
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let _file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) if name.starts_with("krew.log.") => name,
            _ => continue,
        };

        if let Ok(metadata) = entry.metadata() {
            let modified = metadata
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if modified < cutoff {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// Set up the terminal with a dynamic inline viewport.
///
/// No alternate screen — messages are inserted above the viewport and
/// scroll into the terminal's normal scrollback buffer.
fn setup_terminal() -> io::Result<custom_terminal::Terminal> {
    // Enable bracketed paste so multi-line pastes arrive as a single
    // Event::Paste instead of individual key events.
    execute!(stdout(), EnableBracketedPaste)?;

    enable_raw_mode()?;

    // Keyboard enhancement is optional — some terminals (legacy Windows console)
    // don't support it. Attempt but continue gracefully if unsupported.
    let _ = execute!(
        stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        ),
    );

    let mut terminal = custom_terminal::Terminal::new()?;

    // Reserve initial viewport space (separator + 1-line input + separator + status).
    terminal.ensure_viewport_height(4)?;

    Ok(terminal)
}

/// Restore the terminal to its original state.
fn restore_terminal() {
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    let _ = execute!(stdout(), DisableBracketedPaste);
    let _ = disable_raw_mode();
}

/// Load configuration from file, falling back to defaults.
///
/// When `--config` is explicitly provided, the file MUST exist (error if not).
/// When using the default path, a missing file silently falls back to defaults.
fn load_config(cwd: &Path, cli: &Cli) -> anyhow::Result<Config> {
    let explicit = cli.config.is_some();
    let config_path = match &cli.config {
        Some(p) => PathBuf::from(p),
        None => cwd.join(krew_config::CONFIG_FILENAME),
    };

    let mut config = if config_path.exists() {
        tracing::info!(path = %config_path.display(), "Loading config");
        Config::load(&config_path)
            .map_err(|e| anyhow::anyhow!("Failed to load {}: {e}", config_path.display()))?
    } else if explicit {
        anyhow::bail!("Config file not found: {}", config_path.display());
    } else {
        tracing::info!("Config file not found, using defaults");
        Config::default()
    };

    config
        .apply_cli_overrides(cli.agents.as_deref(), cli.approval_mode.as_deref())
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    config.validate().map_err(|e| anyhow::anyhow!("{e}"))?;

    tracing::info!(
        agents = config.agents.len(),
        approval_mode = %config.settings.approval_mode,
        "Config loaded"
    );

    Ok(config)
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    // Initialize logging — _guard must live until program exit.
    let _guard = init_logging(&cwd, cli.verbose)?;
    tracing::info!("krew starting");

    // Load configuration (before terminal setup so errors print normally).
    let config = load_config(&cwd, &cli)?;

    // Build tokio runtime with configurable worker thread count.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.settings.worker_threads)
        .enable_all()
        .build()?;

    tracing::info!(
        worker_threads = config.settings.worker_threads,
        "Tokio runtime created"
    );

    runtime.block_on(async_main(config, cwd))
}

async fn async_main(config: Config, cwd: PathBuf) -> anyhow::Result<()> {
    // Install panic hook that restores the terminal before printing the panic.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    // Set up TUI terminal.
    let mut terminal = setup_terminal()?;

    // Run the application.
    let mut app = app::App::new(cwd, config)?;

    // Collect startup warnings from config normalization.
    let appended = app.config.normalize();
    if !appended.is_empty() {
        let names = appended.join(", ");
        app.startup_warnings.push(format!(
            "settings.reply_order is missing agents, auto-appended: {names}"
        ));
    }

    let result = app.run(&mut terminal).await;

    // Move cursor below the viewport while still in raw mode.
    // In raw mode, \r\n forces the terminal to scroll if at the bottom.
    let viewport_h = terminal.viewport_area.height;
    let newlines = "\r\n".repeat(viewport_h as usize);
    execute!(stdout(), crossterm::style::Print(newlines))?;

    // Restore terminal.
    restore_terminal();

    result
}
