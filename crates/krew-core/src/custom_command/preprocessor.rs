//! Bash preprocessing for custom commands.
//!
//! Scans command text for `!`command`` patterns and executes each via
//! the system shell, replacing each block with its stdout output.

/// Execute all bash preprocessing blocks in the given text.
///
/// Pattern: `` !`command` `` — the command between backticks after `!` is
/// executed, and the entire `` !`...` `` block is replaced with stdout.
///
/// On failure, the block is replaced with an error message.
pub async fn preprocess(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(start) = remaining.find("!`") {
        // Add text before this block.
        result.push_str(&remaining[..start]);

        let after_open = &remaining[start + 2..];
        let Some(close) = after_open.find('`') else {
            // No closing backtick — keep the rest as-is.
            result.push_str(&remaining[start..]);
            remaining = "";
            break;
        };

        let command = &after_open[..close];
        let output = execute_shell(command).await;
        result.push_str(&output);

        remaining = &after_open[close + 1..];
    }

    result.push_str(remaining);
    result
}

/// Execute a shell command and return its output or error message.
async fn execute_shell(command: &str) -> String {
    let shell_result = if cfg!(target_os = "windows") {
        tokio::process::Command::new("cmd")
            .args(["/C", command])
            .output()
            .await
    } else {
        tokio::process::Command::new("sh")
            .args(["-c", command])
            .output()
            .await
    };

    match shell_result {
        Ok(output) => {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout)
                    .trim_end()
                    .to_string()
            } else {
                let code = output.status.code().unwrap_or(-1);
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stderr.is_empty() {
                    format!("[Error: command failed (exit {code})]")
                } else {
                    format!("[Error: command failed (exit {code}): {stderr}]")
                }
            }
        }
        Err(e) => {
            format!("[Error: failed to execute: {command}: {e}]")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_no_bash_blocks() {
        let text = "Hello world, no commands here";
        assert_eq!(preprocess(text).await, text);
    }

    #[tokio::test]
    async fn test_successful_command() {
        let text = "Result: !`echo hello`";
        let result = preprocess(text).await;
        assert_eq!(result, "Result: hello");
    }

    #[tokio::test]
    async fn test_multiple_blocks() {
        let text = "A: !`echo one` B: !`echo two`";
        let result = preprocess(text).await;
        assert_eq!(result, "A: one B: two");
    }

    #[tokio::test]
    async fn test_failed_command() {
        let text = "!`nonexistent_command_xyz_123`";
        let result = preprocess(text).await;
        assert!(
            result.contains("[Error:"),
            "Expected error message, got: {result}"
        );
    }
}
