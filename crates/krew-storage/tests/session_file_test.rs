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
