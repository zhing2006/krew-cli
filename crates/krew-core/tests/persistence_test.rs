use std::collections::HashMap;
use std::path::Path;

use chrono::Utc;
use tempfile::TempDir;

use krew_core::persistence::{SessionSnapshot, build_session_file, load_session_from_disk};
use krew_llm::{ChatMessage, ChatRole, ThinkingBlock};

#[test]
fn thinking_blocks_roundtrip_through_toml() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("roundtrip.toml");

    let blocks = vec![
        ThinkingBlock::Thinking {
            text: "first chunk".to_string(),
            signature: "sig-aaa".to_string(),
        },
        ThinkingBlock::Redacted {
            data: "opaque-blob".to_string(),
        },
        ThinkingBlock::Thinking {
            text: "second chunk".to_string(),
            signature: "sig-bbb".to_string(),
        },
    ];

    let mut assistant = ChatMessage::text(
        ChatRole::Assistant,
        "final reply",
        Some("claude".to_string()),
    );
    assistant.thinking_blocks = blocks.clone();

    let user = ChatMessage::text(ChatRole::User, "do it", None);
    let messages = vec![user, assistant];

    let snapshot = SessionSnapshot {
        session_id: "abcdef12",
        cwd: Path::new("/tmp/project"),
        agent_names: vec!["claude".to_string()],
        messages: &messages,
        token_usage: &HashMap::new(),
        created_at: Utc::now(),
    };
    let session_file = build_session_file(&snapshot);
    krew_storage::session_file::save_session(&path, &session_file).unwrap();

    let restored = load_session_from_disk(&path).unwrap();
    let restored_assistant = restored
        .messages
        .iter()
        .find(|m| m.role == ChatRole::Assistant)
        .expect("assistant message survives roundtrip");
    assert_eq!(restored_assistant.thinking_blocks, blocks);
}

#[test]
fn empty_thinking_blocks_round_trip_to_empty_vec() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty.toml");

    let assistant = ChatMessage::text(ChatRole::Assistant, "plain reply", Some("gpt".to_string()));
    let messages = vec![assistant];

    let snapshot = SessionSnapshot {
        session_id: "empty123",
        cwd: Path::new("/tmp"),
        agent_names: vec!["gpt".to_string()],
        messages: &messages,
        token_usage: &HashMap::new(),
        created_at: Utc::now(),
    };
    let session_file = build_session_file(&snapshot);
    krew_storage::session_file::save_session(&path, &session_file).unwrap();

    let written = std::fs::read_to_string(&path).unwrap();
    assert!(!written.contains("thinking_blocks"));

    let restored = load_session_from_disk(&path).unwrap();
    let restored_assistant = restored
        .messages
        .iter()
        .find(|m| m.role == ChatRole::Assistant)
        .unwrap();
    assert!(restored_assistant.thinking_blocks.is_empty());
}
