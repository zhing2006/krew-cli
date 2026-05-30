use std::fs;
use std::path::Path;

use chrono::Utc;
use tempfile::TempDir;

use krew_storage::session_file::*;

fn make_test_session() -> SessionFile {
    let now = Utc::now();
    SessionFile {
        session: SessionMeta {
            id: "test1234".to_string(),
            cwd: "/tmp/project".to_string(),
            agents: vec!["gpt".to_string(), "opus".to_string()],
            total_tokens_used: 1500,
            created_at: now,
            updated_at: now,
        },
        messages: vec![
            MessageEntry {
                role: "user".to_string(),
                agent_name: None,
                addressee: Some("all".to_string()),
                content: "hello world".to_string(),
                usage: None,
                tool_calls: None,
                tool_call_id: None,
                server_tool_uses: vec![],
                whisper_targets: None,
                thinking_blocks: None,
                raw_content_blocks_json: None,
                created_at: now,
            },
            MessageEntry {
                role: "assistant".to_string(),
                agent_name: Some("gpt".to_string()),
                addressee: None,
                content: "hi there".to_string(),
                usage: Some(UsageEntry {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                }),
                tool_calls: None,
                tool_call_id: None,
                server_tool_uses: vec![],
                whisper_targets: None,
                thinking_blocks: None,
                raw_content_blocks_json: None,
                created_at: now,
            },
        ],
    }
}

#[test]
fn test_save_load_roundtrip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("sessions").join("test1234.toml");

    let session = make_test_session();
    save_session(&path, &session).unwrap();
    let loaded = load_session(&path).unwrap();

    assert_eq!(loaded.session.id, "test1234");
    assert_eq!(loaded.session.agents, vec!["gpt", "opus"]);
    assert_eq!(loaded.session.total_tokens_used, 1500);
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[0].role, "user");
    assert_eq!(loaded.messages[0].content, "hello world");
    assert_eq!(loaded.messages[1].role, "assistant");
    assert_eq!(loaded.messages[1].agent_name.as_deref(), Some("gpt"));
    assert_eq!(
        loaded.messages[1].usage.as_ref().unwrap().prompt_tokens,
        100
    );
}

#[test]
fn test_save_empty_session() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty.toml");

    let session = SessionFile {
        session: SessionMeta {
            id: "empty123".to_string(),
            cwd: "/tmp".to_string(),
            agents: vec![],
            total_tokens_used: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        },
        messages: vec![],
    };

    save_session(&path, &session).unwrap();
    let loaded = load_session(&path).unwrap();
    assert_eq!(loaded.session.id, "empty123");
    assert!(loaded.messages.is_empty());
}

#[test]
fn test_load_nonexistent() {
    let result = load_session(Path::new("/nonexistent/session.toml"));
    assert!(result.is_err());
}

#[test]
fn test_load_corrupted() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("bad.toml");
    fs::write(&path, "this is not valid toml session data [[[").unwrap();
    let result = load_session(&path);
    assert!(result.is_err());
}

#[test]
fn test_list_sessions() {
    let dir = TempDir::new().unwrap();
    let sessions_dir = dir.path().join("sessions");
    fs::create_dir_all(&sessions_dir).unwrap();

    let now = Utc::now();
    let older = now - chrono::Duration::hours(1);

    let s1 = SessionFile {
        session: SessionMeta {
            id: "sess_a".to_string(),
            cwd: "/tmp".to_string(),
            agents: vec!["gpt".to_string()],
            total_tokens_used: 100,
            created_at: older,
            updated_at: older,
        },
        messages: vec![MessageEntry {
            role: "user".to_string(),
            agent_name: None,
            addressee: None,
            content: "first session message".to_string(),
            usage: None,
            tool_calls: None,
            tool_call_id: None,
            server_tool_uses: vec![],
            whisper_targets: None,
            thinking_blocks: None,
            raw_content_blocks_json: None,
            created_at: older,
        }],
    };

    let s2 = SessionFile {
        session: SessionMeta {
            id: "sess_b".to_string(),
            cwd: "/tmp".to_string(),
            agents: vec!["opus".to_string()],
            total_tokens_used: 200,
            created_at: now,
            updated_at: now,
        },
        messages: vec![MessageEntry {
            role: "user".to_string(),
            agent_name: None,
            addressee: None,
            content: "second session message".to_string(),
            usage: None,
            tool_calls: None,
            tool_call_id: None,
            server_tool_uses: vec![],
            whisper_targets: None,
            thinking_blocks: None,
            raw_content_blocks_json: None,
            created_at: now,
        }],
    };

    save_session(&sessions_dir.join("sess_a.toml"), &s1).unwrap();
    save_session(&sessions_dir.join("sess_b.toml"), &s2).unwrap();

    // Also create a corrupted file — should be skipped.
    fs::write(sessions_dir.join("bad.toml"), "not valid").unwrap();

    let summaries = list_sessions(&sessions_dir).unwrap();
    assert_eq!(summaries.len(), 2);
    // Most recent first.
    assert_eq!(summaries[0].id, "sess_b");
    assert_eq!(summaries[1].id, "sess_a");
    assert_eq!(
        summaries[0].first_message_preview.as_deref(),
        Some("second session message")
    );
}

#[test]
fn test_list_sessions_empty_dir() {
    let dir = TempDir::new().unwrap();
    let summaries = list_sessions(dir.path()).unwrap();
    assert!(summaries.is_empty());
}

#[test]
fn test_list_sessions_nonexistent_dir() {
    let summaries = list_sessions(Path::new("/nonexistent/dir")).unwrap();
    assert!(summaries.is_empty());
}

#[test]
fn test_whisper_targets_roundtrip() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("whisper.toml");

    let now = Utc::now();
    let session = SessionFile {
        session: SessionMeta {
            id: "whisper123".to_string(),
            cwd: "/tmp".to_string(),
            agents: vec!["opus".to_string(), "gemini".to_string()],
            total_tokens_used: 0,
            created_at: now,
            updated_at: now,
        },
        messages: vec![
            MessageEntry {
                role: "user".to_string(),
                agent_name: None,
                addressee: Some("opus".to_string()),
                content: "secret message".to_string(),
                usage: None,
                tool_calls: None,
                tool_call_id: None,
                server_tool_uses: vec![],
                whisper_targets: Some(vec!["opus".to_string()]),
                thinking_blocks: None,
                raw_content_blocks_json: None,
                created_at: now,
            },
            MessageEntry {
                role: "assistant".to_string(),
                agent_name: Some("opus".to_string()),
                addressee: None,
                content: "secret reply".to_string(),
                usage: None,
                tool_calls: None,
                tool_call_id: None,
                server_tool_uses: vec![],
                whisper_targets: Some(vec!["opus".to_string()]),
                thinking_blocks: None,
                raw_content_blocks_json: None,
                created_at: now,
            },
            MessageEntry {
                role: "user".to_string(),
                agent_name: None,
                addressee: Some("all".to_string()),
                content: "public message".to_string(),
                usage: None,
                tool_calls: None,
                tool_call_id: None,
                server_tool_uses: vec![],
                whisper_targets: None,
                thinking_blocks: None,
                raw_content_blocks_json: None,
                created_at: now,
            },
        ],
    };

    save_session(&path, &session).unwrap();
    let loaded = load_session(&path).unwrap();

    assert_eq!(loaded.messages.len(), 3);
    assert_eq!(
        loaded.messages[0].whisper_targets,
        Some(vec!["opus".to_string()])
    );
    assert_eq!(
        loaded.messages[1].whisper_targets,
        Some(vec!["opus".to_string()])
    );
    assert!(loaded.messages[2].whisper_targets.is_none());
}

#[test]
fn test_atomic_write_creates_no_tmp_file() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("atomic.toml");

    let session = make_test_session();
    save_session(&path, &session).unwrap();

    // .tmp file should not remain.
    let tmp_path = path.with_extension("toml.tmp");
    assert!(!tmp_path.exists());
    assert!(path.exists());
}

#[test]
fn test_thinking_blocks_roundtrip_with_both_variants() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("thinking.toml");

    let now = Utc::now();
    let session = SessionFile {
        session: SessionMeta {
            id: "think123".to_string(),
            cwd: "/tmp".to_string(),
            agents: vec!["claude".to_string()],
            total_tokens_used: 0,
            created_at: now,
            updated_at: now,
        },
        messages: vec![MessageEntry {
            role: "assistant".to_string(),
            agent_name: Some("claude".to_string()),
            addressee: None,
            content: "answer".to_string(),
            usage: None,
            tool_calls: None,
            tool_call_id: None,
            server_tool_uses: vec![],
            whisper_targets: None,
            thinking_blocks: Some(vec![
                ThinkingBlockEntry::Thinking {
                    text: "step one".to_string(),
                    signature: "sig-1".to_string(),
                },
                ThinkingBlockEntry::RedactedThinking {
                    data: "opaque".to_string(),
                },
            ]),
            raw_content_blocks_json: None,
            created_at: now,
        }],
    };

    save_session(&path, &session).unwrap();
    let loaded = load_session(&path).unwrap();
    let blocks = loaded.messages[0].thinking_blocks.as_ref().unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(
        blocks[0],
        ThinkingBlockEntry::Thinking {
            text: "step one".to_string(),
            signature: "sig-1".to_string(),
        }
    );
    assert_eq!(
        blocks[1],
        ThinkingBlockEntry::RedactedThinking {
            data: "opaque".to_string(),
        }
    );

    // Spot-check the on-disk TOML shape: redacted blocks must serialise with
    // the discriminator tag and `data` only — no `text` / `signature` keys
    // leaking from the Thinking variant.
    let raw = fs::read_to_string(&path).unwrap();
    let redacted_section = raw
        .split("[[messages.thinking_blocks]]")
        .find(|chunk| chunk.contains("block_type = \"redacted_thinking\""))
        .expect("redacted thinking_blocks section must be present in TOML");
    let redacted_section = redacted_section
        .split("[[messages.thinking_blocks]]")
        .next()
        .unwrap();
    assert!(
        redacted_section.contains("data = \"opaque\""),
        "redacted block must carry its opaque `data`, got:\n{redacted_section}"
    );
    assert!(
        !redacted_section.contains("text ="),
        "redacted block must not serialise a `text` key, got:\n{redacted_section}"
    );
    assert!(
        !redacted_section.contains("signature ="),
        "redacted block must not serialise a `signature` key, got:\n{redacted_section}"
    );
}

#[test]
fn test_legacy_session_without_thinking_blocks_loads() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("legacy.toml");

    // Hand-written legacy TOML with no `thinking_blocks` key.
    let toml = r#"
[session]
id = "legacy1"
cwd = "/tmp"
agents = ["gpt"]
total_tokens_used = 0
created_at = "2025-01-01T00:00:00Z"
updated_at = "2025-01-01T00:00:00Z"

[[messages]]
role = "assistant"
agent_name = "gpt"
content = "old reply"
created_at = "2025-01-01T00:00:00Z"
"#;
    fs::write(&path, toml).unwrap();

    let loaded = load_session(&path).unwrap();
    assert_eq!(loaded.messages.len(), 1);
    assert!(loaded.messages[0].thinking_blocks.is_none());
    assert_eq!(loaded.messages[0].content, "old reply");
}

#[test]
fn test_empty_thinking_blocks_omits_key_when_none() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty_thinking.toml");
    let session = make_test_session();
    save_session(&path, &session).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(!text.contains("thinking_blocks"));
}

#[test]
fn test_empty_thinking_blocks_omits_key_when_some_empty_vec() {
    // The persistence layer may map an empty ChatMessage.thinking_blocks to
    // `Some(vec![])` rather than `None`; the on-disk form must still drop the
    // key so resumed sessions and externally written sessions stay byte-equal.
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("empty_thinking_some.toml");
    let now = Utc::now();
    let session = SessionFile {
        session: SessionMeta {
            id: "empty_some".to_string(),
            cwd: "/tmp/project".to_string(),
            agents: vec!["claude".to_string()],
            total_tokens_used: 0,
            created_at: now,
            updated_at: now,
        },
        messages: vec![MessageEntry {
            role: "assistant".to_string(),
            agent_name: Some("claude".to_string()),
            addressee: None,
            content: "plain reply".to_string(),
            usage: None,
            tool_calls: None,
            tool_call_id: None,
            server_tool_uses: vec![],
            whisper_targets: None,
            thinking_blocks: Some(vec![]),
            raw_content_blocks_json: None,
            created_at: now,
        }],
    };
    save_session(&path, &session).unwrap();
    let text = fs::read_to_string(&path).unwrap();
    assert!(
        !text.contains("thinking_blocks"),
        "Some(vec![]) must serialise as if absent, got: {text}"
    );
}
