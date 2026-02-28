mod app;
mod render;

use std::io::{self, stdout};
use std::path::Path;

use clap::Parser;
use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::crossterm::execute;
use ratatui::{Terminal, TerminalOptions, Viewport};

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

/// Set up the terminal with a small inline viewport for the input area.
///
/// No alternate screen — messages are inserted above the viewport via
/// `insert_before` and scroll into the terminal's normal scrollback buffer.
fn setup_terminal() -> io::Result<Terminal<CrosstermBackend<io::Stdout>>> {
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

    let backend = CrosstermBackend::new(stdout());

    // Small inline viewport — only holds the input prompt + status bar.
    // All other content (header, messages) is inserted above and scrolls
    // into the terminal's scrollback buffer naturally.
    let terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Inline(4),
        },
    )?;

    Ok(terminal)
}

/// Restore the terminal to its original state.
fn restore_terminal() {
    let _ = execute!(stdout(), PopKeyboardEnhancementFlags);
    let _ = disable_raw_mode();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    // Initialize logging — _guard must live until program exit.
    let _guard = init_logging(&cwd, cli.verbose)?;
    tracing::info!("krew starting");

    // Install panic hook that restores the terminal before printing the panic.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));

    // Set up TUI terminal.
    let mut terminal = setup_terminal()?;

    // Run the application.
    let result = app::App::new(cwd)?.run(&mut terminal).await;

    // Move cursor below the viewport while still in raw mode.
    // In raw mode, \r\n forces the terminal to scroll if at the bottom.
    execute!(stdout(), crossterm::style::Print("\r\n\r\n\r\n\r\n"),)?;

    // Restore terminal.
    restore_terminal();

    result
}
