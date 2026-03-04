use krew_tools::ToolContext;
use krew_tools::ToolHandler;
use krew_tools::builtin::ShellTool;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn echo_command() {
    let dir = TempDir::new().unwrap();
    let tool = ShellTool::new(dir.path().to_path_buf());
    let ctx = ToolContext::default();

    let result = tool
        .execute(json!({ "command": "echo hello" }), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("hello"));
}

#[tokio::test]
async fn echo_command_streams_output() {
    let dir = TempDir::new().unwrap();
    let tool = ShellTool::new(dir.path().to_path_buf());
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let ctx = ToolContext {
        output_tx: Some(tx),
    };

    let result = tool
        .execute(json!({ "command": "echo hello" }), &ctx)
        .await
        .unwrap();

    assert!(!result.is_error);
    let line = rx.try_recv().unwrap();
    assert!(line.contains("hello"));
}

#[tokio::test]
async fn failing_command() {
    let dir = TempDir::new().unwrap();
    let tool = ShellTool::new(dir.path().to_path_buf());
    let ctx = ToolContext::default();

    let result = tool
        .execute(json!({ "command": "exit 42" }), &ctx)
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("42"));
}

#[tokio::test]
async fn timeout_kills_command() {
    let dir = TempDir::new().unwrap();
    let tool = ShellTool::new(dir.path().to_path_buf());
    let ctx = ToolContext::default();

    let result = tool
        .execute(
            json!({
                "command": "sleep 5",
                "timeout_seconds": 1
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("timed out"));
}

#[tokio::test]
async fn requires_approval_returns_true() {
    let dir = TempDir::new().unwrap();
    let tool = ShellTool::new(dir.path().to_path_buf());
    assert!(tool.requires_approval());
}
