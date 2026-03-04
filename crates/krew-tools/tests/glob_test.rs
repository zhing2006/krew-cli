use krew_tools::ToolContext;
use krew_tools::ToolHandler;
use krew_tools::builtin::GlobTool;
use serde_json::json;
use tempfile::TempDir;

fn setup_test_tree() -> TempDir {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("main.rs"), "fn main() {}").unwrap();
    std::fs::write(src.join("lib.rs"), "pub mod lib;").unwrap();
    std::fs::write(dir.path().join("README.md"), "# readme").unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
    dir
}

#[tokio::test]
async fn matches_rust_files() {
    let dir = setup_test_tree();
    let tool = GlobTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(json!({ "pattern": "**/*.rs" }), &ToolContext::default())
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("main.rs"));
    assert!(result.content.contains("lib.rs"));
    assert!(!result.content.contains("README"));
    assert!(result.content.contains("(2 files)"));
}

#[tokio::test]
async fn matches_specific_directory() {
    let dir = setup_test_tree();
    let tool = GlobTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({ "pattern": "*.rs", "path": "src" }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("main.rs"));
}

#[tokio::test]
async fn no_matches() {
    let dir = setup_test_tree();
    let tool = GlobTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(json!({ "pattern": "**/*.py" }), &ToolContext::default())
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("No files matched"));
}

#[tokio::test]
async fn invalid_pattern() {
    let dir = setup_test_tree();
    let tool = GlobTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(json!({ "pattern": "[invalid" }), &ToolContext::default())
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn respects_limit() {
    let dir = setup_test_tree();
    let tool = GlobTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({ "pattern": "**/*", "limit": 2 }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    // Should have at most 2 results.
    let lines: Vec<&str> = result
        .content
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('('))
        .collect();
    assert!(lines.len() <= 2);
}
