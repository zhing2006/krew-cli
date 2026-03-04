use krew_tools::ToolContext;
use krew_tools::ToolHandler;
use krew_tools::builtin::WriteFileTool;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn creates_new_file() {
    let dir = TempDir::new().unwrap();
    let tool = WriteFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": "hello.txt",
                "content": "Hello, world!\n"
            }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let content = std::fs::read_to_string(dir.path().join("hello.txt")).unwrap();
    assert_eq!(content, "Hello, world!\n");
}

#[tokio::test]
async fn creates_parent_directories() {
    let dir = TempDir::new().unwrap();
    let tool = WriteFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": "deep/nested/dir/file.rs",
                "content": "fn main() {}\n"
            }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(dir.path().join("deep/nested/dir/file.rs").exists());
}

#[tokio::test]
async fn overwrites_existing_file() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("existing.txt"), "old content").unwrap();
    let tool = WriteFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": "existing.txt",
                "content": "new content"
            }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    let content = std::fs::read_to_string(dir.path().join("existing.txt")).unwrap();
    assert_eq!(content, "new content");
}

#[tokio::test]
async fn rejects_path_outside_workspace() {
    let dir = TempDir::new().unwrap();
    let tool = WriteFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": "/etc/shadow",
                "content": "hacked"
            }),
            &ToolContext::default(),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn requires_approval_returns_true() {
    let dir = TempDir::new().unwrap();
    let tool = WriteFileTool::new(dir.path().to_path_buf());
    assert!(tool.requires_approval());
}
