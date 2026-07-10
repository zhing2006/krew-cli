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
    let tool = GrepTool::new(dir.path().to_path_buf(), true);

    let result = tool
        .execute(json!({ "pattern": "TODO" }), &ToolContext::default())
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains("TODO"));
    // Should find in main.rs, lib.rs, and README.md
}

#[tokio::test]
async fn truncates_multibyte_matching_line_at_utf8_boundary() {
    let dir = TempDir::new().unwrap();
    let ascii_prefix = format!("MATCH{}", "a".repeat(494));
    std::fs::write(
        dir.path().join("long.txt"),
        format!("{ascii_prefix}指tail\n"),
    )
    .unwrap();
    let tool = GrepTool::new(dir.path().to_path_buf(), true);

    let result = tool
        .execute(json!({ "pattern": "MATCH" }), &ToolContext::default())
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.content.contains(&format!("{ascii_prefix}...")));
    assert!(!result.content.contains("指tail"));
}

#[tokio::test]
async fn filters_by_include_glob() {
    let dir = setup_test_files();
    let tool = GrepTool::new(dir.path().to_path_buf(), true);

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
    let tool = GrepTool::new(dir.path().to_path_buf(), true);

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
    let tool = GrepTool::new(dir.path().to_path_buf(), true);

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
    let tool = GrepTool::new(dir.path().to_path_buf(), true);

    let result = tool
        .execute(json!({ "pattern": "[invalid" }), &ToolContext::default())
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn searches_specific_path() {
    let dir = setup_test_files();
    let tool = GrepTool::new(dir.path().to_path_buf(), true);

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

#[tokio::test]
async fn allows_search_outside_workspace_when_unrestricted() {
    // Create workspace and an outside directory with a searchable file.
    let workspace = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    std::fs::write(outside.path().join("target.txt"), "findme_unique_marker\n").unwrap();

    // With restrict_workspace=true, searching outside should find nothing.
    let tool_restricted = GrepTool::new(workspace.path().to_path_buf(), true);
    let result = tool_restricted
        .execute(
            json!({ "pattern": "findme_unique_marker", "path": outside.path().to_str().unwrap() }),
            &ToolContext::default(),
        )
        .await;
    // Should fail because path is outside workspace.
    assert!(result.is_err());

    // With restrict_workspace=false, searching outside should work.
    let tool_unrestricted = GrepTool::new(workspace.path().to_path_buf(), false);
    let result = tool_unrestricted
        .execute(
            json!({ "pattern": "findme_unique_marker", "path": outside.path().to_str().unwrap() }),
            &ToolContext::default(),
        )
        .await
        .unwrap();
    assert!(!result.is_error);
    assert!(result.content.contains("findme_unique_marker"));
}
