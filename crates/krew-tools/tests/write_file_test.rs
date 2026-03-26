use krew_tools::ToolContext;
use krew_tools::ToolHandler;
use krew_tools::builtin::WriteFileTool;
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn creates_new_file() {
    let dir = TempDir::new().unwrap();
    let tool = WriteFileTool::new(dir.path().to_path_buf(), true);

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
    let tool = WriteFileTool::new(dir.path().to_path_buf(), true);

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
    let tool = WriteFileTool::new(dir.path().to_path_buf(), true);

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
    let tool = WriteFileTool::new(dir.path().to_path_buf(), true);

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
async fn allows_path_outside_workspace_when_unrestricted() {
    let workspace = std::env::temp_dir().join("krew_write_unrestricted_ws");
    let _ = tokio::fs::create_dir_all(&workspace).await;
    let outside = std::env::temp_dir().join("krew_write_unrestricted_out");
    let _ = tokio::fs::create_dir_all(&outside).await;

    let tool = WriteFileTool::new(workspace.clone(), false);
    let target = outside.join("test.txt");
    let args = serde_json::json!({
        "file_path": target.to_str().unwrap(),
        "content": "hello from outside"
    });
    let ctx = krew_tools::ToolContext::default();
    let result = tool.execute(args, &ctx).await;
    assert!(result.is_ok());
    assert!(!result.unwrap().is_error);

    let _ = tokio::fs::remove_dir_all(&workspace).await;
    let _ = tokio::fs::remove_dir_all(&outside).await;
}

#[tokio::test]
async fn requires_approval_returns_true() {
    let dir = TempDir::new().unwrap();
    let tool = WriteFileTool::new(dir.path().to_path_buf(), true);
    assert!(tool.requires_approval());
}
