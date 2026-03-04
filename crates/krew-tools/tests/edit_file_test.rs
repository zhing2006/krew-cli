use std::io::Write;
use std::path::PathBuf;

use krew_tools::ToolContext;
use krew_tools::ToolHandler;
use krew_tools::builtin::EditFileTool;
use serde_json::json;
use tempfile::TempDir;

fn setup_test_file(content: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.rs");
    let mut f = std::fs::File::create(&file_path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    (dir, file_path)
}

#[tokio::test]
async fn single_replacement() {
    let (dir, file_path) = setup_test_file("fn main() {\n    println!(\"hello\");\n}\n");
    let tool = EditFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "println!(\"hello\")",
                "new_string": "println!(\"world\")"
            }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("-    println!(\"hello\")"));
    assert!(result.content.contains("+    println!(\"world\")"));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert!(content.contains("println!(\"world\")"));
    assert!(!content.contains("println!(\"hello\")"));
}

#[tokio::test]
async fn old_string_not_found() {
    let (dir, file_path) = setup_test_file("fn main() {}\n");
    let tool = EditFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "nonexistent text",
                "new_string": "replacement"
            }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("not found"));
}

#[tokio::test]
async fn multiple_matches_error() {
    let (dir, file_path) = setup_test_file("aaa\nbbb\naaa\n");
    let tool = EditFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "aaa",
                "new_string": "ccc"
            }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(result.is_error);
    assert!(result.content.contains("2 times"));
}

#[tokio::test]
async fn file_not_found() {
    let dir = TempDir::new().unwrap();
    let tool = EditFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": "nonexistent.rs",
                "old_string": "a",
                "new_string": "b"
            }),
            &ToolContext::default(),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn rejects_path_outside_workspace() {
    let dir = TempDir::new().unwrap();
    let tool = EditFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": "/etc/passwd",
                "old_string": "root",
                "new_string": "hacked"
            }),
            &ToolContext::default(),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn diff_output_format() {
    let (dir, file_path) = setup_test_file("line1\nline2\nline3\n");
    let tool = EditFileTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({
                "file_path": file_path.to_str().unwrap(),
                "old_string": "line2",
                "new_string": "modified"
            }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    // Should contain unified diff markers.
    assert!(result.content.contains("@@"));
    assert!(result.content.contains("-line2"));
    assert!(result.content.contains("+modified"));
}
