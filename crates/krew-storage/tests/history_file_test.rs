use std::path::Path;

use tempfile::TempDir;

use krew_storage::history_file::*;

#[test]
fn test_escape_unescape_roundtrip() {
    let cases = [
        "hello world",
        "line1\nline2",
        "line1\nline2\nline3",
        "path\\to\\file",
        "mixed\\path\nand\nnewlines",
        "",
        "no special chars",
        "trailing backslash\\",
        "\\n literal",
    ];
    for original in &cases {
        let escaped = escape_line(original);
        let unescaped = unescape_line(&escaped);
        assert_eq!(original, &unescaped, "roundtrip failed for: {original:?}");
        // Escaped form must not contain raw newlines.
        assert!(
            !escaped.contains('\n'),
            "escaped form contains newline for: {original:?}"
        );
    }
}

#[test]
fn test_load_nonexistent_file() {
    let result = load_history(Path::new("/nonexistent/history"));
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_save_load_roundtrip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history");

    let entries = vec![
        "hello".to_string(),
        "multi\nline\ninput".to_string(),
        "back\\slash".to_string(),
    ];

    save_history(&path, &entries).unwrap();
    let loaded = load_history(&path).unwrap();
    assert_eq!(entries, loaded);
}

#[test]
fn test_append_and_load() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history");

    append_history_entry(&path, "first").unwrap();
    append_history_entry(&path, "second\nline").unwrap();
    append_history_entry(&path, "third").unwrap();

    let loaded = load_history(&path).unwrap();
    assert_eq!(loaded, vec!["first", "second\nline", "third"]);
}

#[test]
fn test_save_truncates() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("history");

    // Write 5 entries.
    let entries: Vec<String> = (0..5).map(|i| format!("entry{i}")).collect();
    save_history(&path, &entries).unwrap();

    // Truncate to last 3.
    let truncated = entries[2..].to_vec();
    save_history(&path, &truncated).unwrap();

    let loaded = load_history(&path).unwrap();
    assert_eq!(loaded, vec!["entry2", "entry3", "entry4"]);
}
