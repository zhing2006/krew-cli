use std::io::Write;
use std::path::PathBuf;

use krew_tools::ToolHandler;
use krew_tools::builtin::ReadFileTool;
use krew_tools::ToolContext;
use serde_json::json;
use tempfile::TempDir;

fn setup_test_file(content: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.txt");
    let mut f = std::fs::File::create(&file_path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    (dir, file_path)
}

#[tokio::test]
async fn reads_full_file() {
    let (dir, file_path) = setup_test_file("alpha\nbeta\ngamma\n");
    let tool = ReadFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({ "file_path": file_path.to_str().unwrap() }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("L1: alpha"));
    assert!(result.content.contains("L2: beta"));
    assert!(result.content.contains("L3: gamma"));
    assert!(result.content.contains("(3 lines)"));
}

#[tokio::test]
async fn reads_with_offset_and_limit() {
    let (dir, file_path) = setup_test_file("first\nsecond\nthird\nfourth\n");
    let tool = ReadFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": file_path.to_str().unwrap(),
                "offset": 2,
                "limit": 2
            }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("L2: second"));
    assert!(result.content.contains("L3: third"));
    assert!(!result.content.contains("L1:"));
    assert!(!result.content.contains("L4:"));
}

#[tokio::test]
async fn offset_exceeds_file_length() {
    let (dir, file_path) = setup_test_file("only\n");
    let tool = ReadFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": file_path.to_str().unwrap(),
                "offset": 10
            }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("exceeds file length"));
}

#[tokio::test]
async fn rejects_path_outside_workspace() {
    let dir = TempDir::new().unwrap();
    let tool = ReadFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({ "file_path": "/etc/passwd" }),
            &ToolContext::default(),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn handles_crlf_line_endings() {
    let (dir, file_path) = setup_test_file("one\r\ntwo\r\n");
    let tool = ReadFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({ "file_path": file_path.to_str().unwrap() }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(result.content.contains("L1: one"));
    assert!(result.content.contains("L2: two"));
}

#[tokio::test]
async fn rejects_binary_file() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("image.png");
    // Write bytes with NUL to simulate binary content.
    std::fs::write(&file_path, b"\x89PNG\r\n\x1a\n\x00\x00\x00").unwrap();
    let tool = ReadFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({ "file_path": file_path.to_str().unwrap() }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("binary file"));
}

#[tokio::test]
async fn invalid_offset_zero() {
    let (dir, file_path) = setup_test_file("test\n");
    let tool = ReadFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": file_path.to_str().unwrap(),
                "offset": 0
            }),
            &ToolContext::default(),
        )
        .await;

    assert!(result.is_err());
}
