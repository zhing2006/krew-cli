//! Shell command execution tool.

use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use tokio::io::{AsyncBufReadExt, BufReader};

use crate::{ToolContext, ToolError, ToolHandler, ToolResult, ToolSpec};

/// Default command timeout in seconds.
const DEFAULT_TIMEOUT_SECS: u64 = 120;
/// Maximum output size in bytes before truncation.
const MAX_OUTPUT_BYTES: usize = 100 * 1024; // 100 KB

fn truncate_output(output: &mut String) {
    if output.len() > MAX_OUTPUT_BYTES {
        let boundary = crate::truncate_utf8(output, MAX_OUTPUT_BYTES).len();
        output.truncate(boundary);
        output.push_str("\n\n[output truncated at 100KB]");
    }
}

/// Built-in tool for executing shell commands.
pub struct ShellTool {
    cwd: PathBuf,
}

#[derive(Deserialize)]
struct ShellArgs {
    command: String,
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

/// Cached shell path and flag.
static SHELL_INFO: OnceLock<Result<(PathBuf, &'static str), String>> = OnceLock::new();

/// Detect the appropriate shell executable for the current platform.
///
/// Replicates Claude Code's Git Bash detection logic on Windows:
///   1. `KREW_BASH_PATH` env var
///   2. Search PATH for bash.exe, skip System32 WSL bash
///   3. Hardcoded Git for Windows paths
///
/// On Unix:
///   1. `KREW_BASH_PATH` env var
///   2. `$SHELL` env var
///   3. `/bin/sh` fallback
fn detect_shell() -> Result<(PathBuf, &'static str), String> {
    // 1. Environment variable override.
    if let Ok(path) = std::env::var("KREW_BASH_PATH") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok((p, "-c"));
        }
        return Err(format!(
            "KREW_BASH_PATH is set to '{}' but the file does not exist",
            path
        ));
    }

    #[cfg(windows)]
    {
        detect_shell_windows()
    }

    #[cfg(not(windows))]
    {
        detect_shell_unix()
    }
}

#[cfg(windows)]
fn detect_shell_windows() -> Result<(PathBuf, &'static str), String> {
    if let Ok(path_var) = std::env::var("PATH") {
        // Derive bash from git.exe in PATH.
        // Git typically puts `<root>/cmd` in PATH; bash lives at `<root>/bin/bash.exe`.
        // Also check if the directory itself contains bash (e.g. `<root>/bin` in PATH).
        for dir in path_var.split(';') {
            let dir_path = PathBuf::from(dir);

            // Direct: bash.exe in a git-related PATH entry.
            if dir.to_lowercase().contains("git") {
                let candidate = dir_path.join("bash.exe");
                if candidate.exists() {
                    return Ok((candidate, "-c"));
                }
            }

            // Sibling: git.exe here → look for ../bin/bash.exe.
            if dir_path.join("git.exe").exists()
                && let Some(git_root) = dir_path.parent()
            {
                let candidate = git_root.join("bin").join("bash.exe");
                if candidate.exists() {
                    return Ok((candidate, "-c"));
                }
            }
        }
    }

    // Hardcoded Git for Windows paths.
    let candidates = [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ];
    for path in &candidates {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok((p, "-c"));
        }
    }

    Err(
        "Git Bash not found. Install Git for Windows or set KREW_BASH_PATH environment variable."
            .to_string(),
    )
}

#[cfg(not(windows))]
fn detect_shell_unix() -> Result<(PathBuf, &'static str), String> {
    // 2. $SHELL env var.
    if let Ok(shell) = std::env::var("SHELL") {
        let p = PathBuf::from(&shell);
        if p.exists() {
            return Ok((p, "-c"));
        }
    }

    // 3. Fallback.
    let sh = PathBuf::from("/bin/sh");
    if sh.exists() {
        return Ok((sh, "-c"));
    }

    Err("No shell found. Set KREW_BASH_PATH environment variable.".to_string())
}

/// Get (or detect and cache) the shell path and flag.
fn get_shell() -> Result<(&'static PathBuf, &'static str), String> {
    let result = SHELL_INFO.get_or_init(detect_shell);
    match result {
        Ok((path, flag)) => Ok((path, flag)),
        Err(e) => Err(e.clone()),
    }
}

/// Read all lines from an async pipe, optionally forwarding each line
/// to a streaming output channel.
async fn read_pipe(
    pipe: Option<impl tokio::io::AsyncRead + Unpin>,
    tx: Option<tokio::sync::mpsc::UnboundedSender<String>>,
) -> String {
    let Some(pipe) = pipe else {
        return String::new();
    };
    let mut lines = BufReader::new(pipe).lines();
    let mut output = String::new();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(ref tx) = tx {
            let _ = tx.send(line.clone());
        }
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&line);
    }
    output
}

impl ShellTool {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }

    pub fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "shell".to_string(),
            description: "Execute a shell command and return stdout/stderr. \
                 Use timeout_seconds for long-running commands (e.g. cargo build may need 300s)."
                .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to execute"
                    },
                    "timeout_seconds": {
                        "type": "number",
                        "description": "Timeout in seconds (default: 120)"
                    }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn requires_approval(&self) -> bool {
        true
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: ShellArgs =
            serde_json::from_value(args).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;

        let (shell_path, shell_flag) = get_shell().map_err(ToolError::Execution)?;

        let timeout = Duration::from_secs(args.timeout_seconds);

        let mut cmd = tokio::process::Command::new(shell_path);
        cmd.arg(shell_flag)
            .arg(&args.command)
            .current_dir(&self.cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // On Windows, prevent console window from flashing.
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| {
            ToolError::Execution(format!(
                "failed to spawn shell '{}': {e}",
                shell_path.display()
            ))
        })?;

        // Take stdout/stderr pipes and spawn reader tasks so output
        // streams to the TUI while `child.wait()` handles the timeout.
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let out_tx_stdout = ctx.output_tx.clone();
        let out_tx_stderr = ctx.output_tx.clone();

        let stdout_handle = tokio::spawn(async move { read_pipe(stdout, out_tx_stdout).await });
        let stderr_handle = tokio::spawn(async move { read_pipe(stderr, out_tx_stderr).await });

        // Wait for process exit with timeout (reliable on Windows).
        let status = match tokio::time::timeout(timeout, child.wait()).await {
            Ok(Ok(status)) => status,
            Ok(Err(e)) => {
                let _ = child.kill().await;
                stdout_handle.abort();
                stderr_handle.abort();
                return Err(ToolError::Execution(format!("shell command failed: {e}")));
            }
            Err(_) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                stdout_handle.abort();
                stderr_handle.abort();
                return Ok(ToolResult {
                    content: format!(
                        "Command timed out after {} seconds. \
                         Use timeout_seconds parameter for long-running commands.",
                        args.timeout_seconds
                    ),
                    is_error: true,
                    images: vec![],
                });
            }
        };

        // Process exited — drain remaining pipe output.
        let stdout_text = stdout_handle.await.unwrap_or_default();
        let stderr_text = stderr_handle.await.unwrap_or_default();

        let mut combined = String::new();
        if !stderr_text.is_empty() {
            combined.push_str(&stderr_text);
        }
        if !stdout_text.is_empty() {
            if !combined.is_empty() {
                combined.push('\n');
            }
            combined.push_str(&stdout_text);
        }

        let exit_code = status.code().unwrap_or(-1);

        // Truncate if too large.
        truncate_output(&mut combined);

        if combined.is_empty() {
            combined = format!("(no output, exit code {exit_code})");
        }

        let is_error = !status.success();
        if is_error && !combined.contains(&format!("exit code {exit_code}")) {
            combined.push_str(&format!("\n\n(exit code {exit_code})"));
        }

        Ok(ToolResult {
            content: combined,
            is_error,
            images: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_OUTPUT_BYTES, truncate_output};

    #[test]
    fn truncate_output_respects_utf8_boundary() {
        let ascii_prefix = "a".repeat(MAX_OUTPUT_BYTES - 1);
        let mut output = format!("{ascii_prefix}指tail");

        truncate_output(&mut output);

        assert_eq!(
            output,
            format!("{ascii_prefix}\n\n[output truncated at 100KB]")
        );
    }
}
