use krew_tools::ToolContext;
use krew_tools::ToolHandler;
use krew_tools::builtin::GrepTool;
use serde_json::json;
use tempfile::TempDir;

fn setup_test_files() -> TempDir {
    let dir = TempDir::new().unwrap();
    let src = dir.path().join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(
        src.join("main.rs"),
        "fn main() {\n    println!(\"hello\");\n    // TODO: fix this\n}\n",
    )
    .unwrap();
    std::fs::write(
        src.join("lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n// TODO: add tests\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("README.md"), "# Project\nNo TODOs here").unwrap();
    dir
}

#[tokio::test]
async fn finds_matching_lines() {
    let dir = setup_test_files();
    let tool = GrepTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(json!({ "pattern": "TODO" }), &ToolContext::default())
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("TODO"));
    // Should find in main.rs, lib.rs, and README.md
}

#[tokio::test]
async fn filters_by_include_glob() {
    let dir = setup_test_files();
    let tool = GrepTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({ "pattern": "TODO", "include": "*.rs" }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains(".rs"));
    assert!(!result.content.contains("README"));
}

#[tokio::test]
async fn respects_limit() {
    let dir = setup_test_files();
    let tool = GrepTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({ "pattern": "TODO", "limit": 1 }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("(1 matches)"));
}

#[tokio::test]
async fn no_matches() {
    let dir = setup_test_files();
    let tool = GrepTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({ "pattern": "NONEXISTENT_PATTERN_xyz123" }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("No matches found"));
}

#[tokio::test]
async fn invalid_regex() {
    let dir = setup_test_files();
    let tool = GrepTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(json!({ "pattern": "[invalid" }), &ToolContext::default())
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn searches_specific_path() {
    let dir = setup_test_files();
    let tool = GrepTool::new(dir.path().to_path_buf());

    let result = tool
        .execute(
            json!({ "pattern": "fn", "path": "src" }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("fn main"));
}
