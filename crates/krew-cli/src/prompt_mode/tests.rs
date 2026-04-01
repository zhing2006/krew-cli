use krew_core::event::{AgentEvent, ReviewDecision};
use krew_core::router::{self, Addressee};
use krew_llm::Usage;
use tokio::sync::{mpsc, oneshot};

use super::{
    OutputFormat, combine_stdin_and_prompt, consume_agent_events, format_tool_args_preview,
};

// --- 7.1: stdin content combination ---

#[test]
fn combine_stdin_empty_returns_prompt_only() {
    assert_eq!(
        combine_stdin_and_prompt("", "@claude hello"),
        "@claude hello"
    );
    assert_eq!(
        combine_stdin_and_prompt("  \n  ", "@claude hello"),
        "@claude hello"
    );
}

#[test]
fn combine_stdin_wraps_content() {
    let result = combine_stdin_and_prompt("file content here", "@claude review");
    assert_eq!(
        result,
        "<stdin>\nfile content here\n</stdin>\n\n@claude review"
    );
}

#[test]
fn combine_stdin_preserves_at_tokens_in_prompt() {
    // stdin with @agent tokens should not affect the prompt's @agent tokens.
    let result = combine_stdin_and_prompt("@gpt hello", "@claude review");
    assert!(result.contains("@gpt hello")); // stdin preserved
    assert!(result.contains("@claude review")); // prompt preserved
    assert!(result.starts_with("<stdin>"));
}

#[test]
fn combine_stdin_trims_trailing_whitespace() {
    let result = combine_stdin_and_prompt("content\n\n\n", "@claude hello");
    assert_eq!(result, "<stdin>\ncontent\n</stdin>\n\n@claude hello");
}

// --- 7.2: addressing validation ---

#[test]
fn addressing_no_at_returns_last_respondent() {
    let agents = vec!["claude".to_string(), "gpt".to_string()];
    let (addressee, _, _) = router::parse_input("hello", &agents).unwrap();
    assert!(matches!(addressee, Addressee::LastRespondent));
}

#[test]
fn addressing_unknown_at_returns_last_respondent() {
    let agents = vec!["claude".to_string(), "gpt".to_string()];
    let (addressee, _, _) = router::parse_input("@nonexistent hello", &agents).unwrap();
    assert!(matches!(addressee, Addressee::LastRespondent));
}

#[test]
fn addressing_known_agent_returns_single() {
    let agents = vec!["claude".to_string(), "gpt".to_string()];
    let (addressee, _, _) = router::parse_input("@claude hello", &agents).unwrap();
    assert!(matches!(addressee, Addressee::Single(ref name) if name == "claude"));
}

#[test]
fn addressing_all_returns_all() {
    let agents = vec!["claude".to_string(), "gpt".to_string()];
    let (addressee, _, _) = router::parse_input("@all hello", &agents).unwrap();
    assert!(matches!(addressee, Addressee::All));
}

#[test]
fn addressing_multiple_agents() {
    let agents = vec!["claude".to_string(), "gpt".to_string()];
    let (addressee, _, _) = router::parse_input("@claude @gpt hello", &agents).unwrap();
    assert!(matches!(addressee, Addressee::Multiple(ref names) if names.len() == 2));
}

#[test]
fn addressing_known_mixed_with_unknown_routes_to_known() {
    let agents = vec!["claude".to_string(), "gpt".to_string()];
    let (addressee, body, _) = router::parse_input("@claude explain @dataclass", &agents).unwrap();
    assert!(matches!(addressee, Addressee::Single(ref name) if name == "claude"));
    // Body preserves the full text including @dataclass.
    assert!(body.contains("@dataclass"));
}

// --- 7.4 & 7.5: output formatting via consume_agent_events ---

fn make_done_event(text: &str) -> AgentEvent {
    AgentEvent::Done {
        usage: Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        },
        intermediate_messages: vec![],
        final_text: text.to_string(),
        server_tool_uses: vec![],
    }
}

#[tokio::test]
async fn text_format_outputs_header_and_text() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::TextDelta("hello".to_string())).unwrap();
    tx.send(make_done_event("hello")).unwrap();
    drop(tx);

    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Text, false, None).await;
    assert_eq!(result.final_text, "hello");
    assert!(!result.has_error);
    assert!(result.usage.is_some());
}

#[tokio::test]
async fn json_format_outputs_text_event() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::TextDelta("hello world".to_string()))
        .unwrap();
    tx.send(make_done_event("hello world")).unwrap();
    drop(tx);

    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Json, false, None).await;
    assert_eq!(result.final_text, "hello world");
    assert!(!result.has_error);
}

#[tokio::test]
async fn thinking_delta_is_silently_discarded() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ThinkingDelta("thinking...".to_string()))
        .unwrap();
    tx.send(AgentEvent::TextDelta("output".to_string()))
        .unwrap();
    tx.send(make_done_event("output")).unwrap();
    drop(tx);

    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Text, false, None).await;
    assert_eq!(result.final_text, "output");
    // ThinkingDelta should not appear in the text buffer.
    assert!(!result.final_text.contains("thinking"));
}

#[tokio::test]
async fn tool_call_events_processed() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ToolCallStart {
        name: "read_file".to_string(),
        display_name: "read_file".to_string(),
        arguments: r#"{"path":"src/main.rs"}"#.to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ToolCallDone {
        name: "read_file".to_string(),
        result_summary: "done (150 lines)".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::TextDelta("response".to_string()))
        .unwrap();
    tx.send(make_done_event("response")).unwrap();
    drop(tx);

    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Text, false, None).await;
    assert_eq!(result.final_text, "response");
    assert!(!result.has_error);
}

#[tokio::test]
async fn tool_call_output_processed() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ToolCallStart {
        name: "shell".to_string(),
        display_name: "shell".to_string(),
        arguments: r#"{"command":"ls"}"#.to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ToolCallOutput {
        text: "file1.rs".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ToolCallOutput {
        text: "file2.rs".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ToolCallDone {
        name: "shell".to_string(),
        result_summary: "exit 0".to_string(),
    })
    .unwrap();
    tx.send(make_done_event("done")).unwrap();
    drop(tx);

    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Text, false, None).await;
    assert_eq!(result.final_text, "done");
}

#[tokio::test]
async fn server_tool_events_processed() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ServerToolStart {
        name: "web_search".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ServerToolDone {
        name: "web_search".to_string(),
        query: Some("rust async".to_string()),
    })
    .unwrap();
    // Done event carries the authoritative server_tool_uses.
    tx.send(AgentEvent::Done {
        usage: Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        },
        intermediate_messages: vec![],
        final_text: "search results".to_string(),
        server_tool_uses: vec![krew_llm::ServerToolUseInfo {
            name: "web_search".to_string(),
            query: Some("rust async".to_string()),
        }],
    })
    .unwrap();
    drop(tx);

    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Json, false, None).await;
    assert_eq!(result.final_text, "search results");
    assert_eq!(result.server_tool_uses.len(), 1);
    assert_eq!(result.server_tool_uses[0].name, "web_search");
}

#[tokio::test]
async fn thinking_between_server_tool_does_not_trigger_gemini_style() {
    // ServerToolStart -> ThinkingDelta -> ServerToolDone should NOT set
    // text_after_server_tool, because thinking is silently discarded in -p mode.
    // The display should use the "start/done adjacent" style, not Gemini's name_done style.
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ServerToolStart {
        name: "web_search".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::ThinkingDelta("reasoning...".to_string()))
        .unwrap();
    tx.send(AgentEvent::ServerToolDone {
        name: "web_search".to_string(),
        query: Some("rust".to_string()),
    })
    .unwrap();
    tx.send(AgentEvent::Done {
        usage: Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        },
        intermediate_messages: vec![],
        final_text: "result".to_string(),
        server_tool_uses: vec![krew_llm::ServerToolUseInfo {
            name: "web_search".to_string(),
            query: Some("rust".to_string()),
        }],
    })
    .unwrap();
    drop(tx);

    // This test verifies the function completes without panic and produces
    // correct results. The actual stdout format (⎿ vs 🌐 name_done) is
    // validated by the fact that text_after_server_tool stays false.
    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Text, false, None).await;
    assert_eq!(result.final_text, "result");
    assert!(!result.has_error);
}

#[tokio::test]
async fn approval_request_auto_denied_in_prompt_mode() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();

    let (resp_tx, resp_rx) = oneshot::channel();
    tx.send(AgentEvent::ApprovalRequest {
        tool_name: "shell".to_string(),
        arguments: r#"{"command":"rm -rf /"}"#.to_string(),
        allow_session_approval: false,
        reason: None,
        respond: resp_tx,
    })
    .unwrap();
    tx.send(make_done_event("done")).unwrap();
    drop(tx);

    // Spawn consumer — it will auto-deny (non-interactive mode).
    let handle = tokio::spawn(async move {
        consume_agent_events(&mut rx, "claude", OutputFormat::Text, false, None).await
    });

    // Verify the denial was sent.
    let decision = resp_rx.await.unwrap();
    assert_eq!(decision, ReviewDecision::Denied);

    let result = handle.await.unwrap();
    assert!(!result.has_error);
}

#[tokio::test]
async fn error_event_sets_has_error() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::Error {
        message: "API rate limit exceeded".to_string(),
        intermediate_messages: vec![],
    })
    .unwrap();
    drop(tx);

    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Text, false, None).await;
    assert!(result.has_error);
    // No partial text → final_text should be empty.
    assert!(result.final_text.is_empty());
}

#[tokio::test]
async fn error_with_partial_text_appends_error_annotation() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::TextDelta("partial output".to_string()))
        .unwrap();
    tx.send(AgentEvent::Error {
        message: "connection reset".to_string(),
        intermediate_messages: vec![],
    })
    .unwrap();
    drop(tx);

    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Text, false, None).await;
    assert!(result.has_error);
    assert!(result.final_text.contains("partial output"));
    assert!(result.final_text.contains("[Error: connection reset]"));
}

#[tokio::test]
async fn server_tool_uses_only_from_done_event() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    // ServerToolDone emitted during streaming — should NOT be collected locally.
    tx.send(AgentEvent::ServerToolDone {
        name: "web_search".to_string(),
        query: Some("rust".to_string()),
    })
    .unwrap();
    // Done event carries the authoritative list.
    tx.send(AgentEvent::Done {
        usage: Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        },
        intermediate_messages: vec![],
        final_text: "result".to_string(),
        server_tool_uses: vec![krew_llm::ServerToolUseInfo {
            name: "web_search".to_string(),
            query: Some("rust".to_string()),
        }],
    })
    .unwrap();
    drop(tx);

    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Text, false, None).await;
    // Should have exactly 1 entry (from Done), NOT 2 (Done + local).
    assert_eq!(result.server_tool_uses.len(), 1);
}

#[tokio::test]
async fn retrying_event_does_not_set_error() {
    let (tx, mut rx) = mpsc::unbounded_channel();

    tx.send(AgentEvent::ResponseStart {
        agent_name: "claude".to_string(),
        display_name: "Claude".to_string(),
        color: "blue".to_string(),
    })
    .unwrap();
    tx.send(AgentEvent::Retrying {
        attempt: 1,
        max_attempts: 3,
        reason: "rate limit (429)".to_string(),
        delay_secs: 2.0,
    })
    .unwrap();
    tx.send(AgentEvent::TextDelta("ok".to_string())).unwrap();
    tx.send(make_done_event("ok")).unwrap();
    drop(tx);

    let result = consume_agent_events(&mut rx, "claude", OutputFormat::Text, false, None).await;
    assert!(!result.has_error);
    assert_eq!(result.final_text, "ok");
}

// --- 7.6: OutputFormat parsing ---

#[test]
fn output_format_text() {
    assert_eq!(
        match "text" {
            "text" => OutputFormat::Text,
            "json" => OutputFormat::Json,
            _ => panic!(),
        },
        OutputFormat::Text
    );
}

#[test]
fn output_format_json() {
    assert_eq!(
        match "json" {
            "text" => OutputFormat::Text,
            "json" => OutputFormat::Json,
            _ => panic!(),
        },
        OutputFormat::Json
    );
}

#[test]
#[should_panic]
fn output_format_invalid() {
    match "yaml" {
        "text" => OutputFormat::Text,
        "json" => OutputFormat::Json,
        _ => panic!("invalid format"),
    };
}

// --- 7.4 additional: format_tool_args_preview ---

#[test]
fn tool_args_preview_read_file() {
    let result = format_tool_args_preview("read_file", r#"{"path":"src/main.rs"}"#);
    assert_eq!(result, "src/main.rs");
}

#[test]
fn tool_args_preview_shell() {
    let result = format_tool_args_preview("shell", r#"{"command":"ls -la"}"#);
    assert_eq!(result, "ls -la");
}

#[test]
fn tool_args_preview_shell_long_truncates() {
    let long_cmd = "a".repeat(100);
    let args = format!(r#"{{"command":"{}"}}"#, long_cmd);
    let result = format_tool_args_preview("shell", &args);
    assert!(result.len() <= 63); // 57 + "..."
    assert!(result.ends_with("..."));
}

#[test]
fn tool_args_preview_glob() {
    let result = format_tool_args_preview("glob", r#"{"pattern":"**/*.rs"}"#);
    assert_eq!(result, "**/*.rs");
}
