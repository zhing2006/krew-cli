use std::io::Write;
use std::path::PathBuf;

use krew_tools::ToolContext;
use krew_tools::ToolHandler;
use krew_tools::builtin::ReadFileTool;
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
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

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
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

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
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

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
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

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
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

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
    let file_path = dir.path().join("binary.dat");
    // Write bytes with NUL to simulate binary content.
    std::fs::write(&file_path, b"\x00\x01\x02\x03\x04\x05").unwrap();
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

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

#[test]
fn validate_path_unrestricted_allows_outside_workspace() {
    let cwd = std::env::temp_dir().join("krew_vp_test");
    std::fs::create_dir_all(&cwd).unwrap();
    // Create a file outside workspace
    let outside = std::env::temp_dir().join("krew_vp_outside");
    std::fs::create_dir_all(&outside).unwrap();
    let outside_file = outside.join("test.txt");
    std::fs::write(&outside_file, "hello").unwrap();

    // With restrict=true, should fail
    let result = krew_tools::validate_path(outside_file.to_str().unwrap(), &cwd, true);
    assert!(result.is_err());

    // With restrict=false, should succeed
    let result = krew_tools::validate_path(outside_file.to_str().unwrap(), &cwd, false);
    assert!(result.is_ok());

    let _ = std::fs::remove_dir_all(&cwd);
    let _ = std::fs::remove_dir_all(&outside);
}

#[tokio::test]
async fn reads_png_image() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.png");
    let fake_png = b"\x89PNG\r\n\x1a\nfake image data";
    std::fs::write(&file_path, fake_png).unwrap();
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

    let result = tool
        .execute(
            json!({ "file_path": file_path.to_str().unwrap() }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(result.content, "[Image: test.png]");
    assert_eq!(result.images.len(), 1);
    assert_eq!(result.images[0].media_type, "image/png");
    assert_eq!(result.images[0].data, fake_png);
}

#[tokio::test]
async fn reads_jpeg_image() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("photo.jpg");
    let fake_jpg = b"\xff\xd8\xff\xe0fake jpeg";
    std::fs::write(&file_path, fake_jpg).unwrap();
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

    let result = tool
        .execute(
            json!({ "file_path": file_path.to_str().unwrap() }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(result.content, "[Image: photo.jpg]");
    assert_eq!(result.images.len(), 1);
    assert_eq!(result.images[0].media_type, "image/jpeg");
}

#[tokio::test]
async fn reads_webp_image() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("pic.webp");
    std::fs::write(&file_path, b"RIFF\x00\x00\x00\x00WEBP").unwrap();
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

    let result = tool
        .execute(
            json!({ "file_path": file_path.to_str().unwrap() }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert_eq!(result.images[0].media_type, "image/webp");
}

#[tokio::test]
async fn image_not_found() {
    let dir = TempDir::new().unwrap();
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

    let result = tool
        .execute(
            json!({ "file_path": "missing.png" }),
            &ToolContext::default(),
        )
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn non_image_extension_uses_text_path() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("data.bmp");
    // BMP is not a supported image format, so it should go through text path.
    // Write text content (no NUL bytes) so it passes binary check.
    std::fs::write(&file_path, b"plain text content").unwrap();
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

    let result = tool
        .execute(
            json!({ "file_path": file_path.to_str().unwrap() }),
            &ToolContext::default(),
        )
        .await
        .unwrap();

    assert!(!result.is_error);
    assert!(result.images.is_empty());
    assert!(result.content.contains("L1:"));
}

#[tokio::test]
async fn invalid_offset_zero() {
    let (dir, file_path) = setup_test_file("test\n");
    let tool = ReadFileTool::new(dir.path().to_path_buf(), true);

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
