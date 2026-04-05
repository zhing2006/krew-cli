use std::pin::Pin;
use std::sync::Arc;

use futures::stream;
use krew_config::{ApprovalMode, SamplingConfig};
use krew_llm::{ChatMessage, ChatRole, LlmClient, StreamEvent, Usage};
use krew_tools::ToolRegistry;

use krew_core::event::{AgentEvent, ApprovalCache};
use krew_core::task::{TaskRequest, run_task, run_task_with_events};

// ---------------------------------------------------------------------------
// Mock LlmClient
// ---------------------------------------------------------------------------

/// A mock LLM client that returns a fixed text response.
struct MockClient {
    response: String,
}

impl MockClient {
    fn new(response: &str) -> Self {
        Self {
            response: response.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for MockClient {
    async fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        _tools: &[krew_llm::ToolDefinition],
        _sampling: &SamplingConfig,
        _on_retry: Option<&(dyn Fn(krew_llm::common::RetryInfo) + Send + Sync)>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>, krew_llm::LlmError> {
        let response = self.response.clone();
        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        };
        Ok(Box::pin(stream::iter(vec![
            StreamEvent::TextDelta(response),
            StreamEvent::Done(usage),
        ])))
    }
}

/// A mock LLM client that returns an error.
struct ErrorClient {
    error_msg: String,
}

impl ErrorClient {
    fn new(msg: &str) -> Self {
        Self {
            error_msg: msg.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for ErrorClient {
    async fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        _tools: &[krew_llm::ToolDefinition],
        _sampling: &SamplingConfig,
        _on_retry: Option<&(dyn Fn(krew_llm::common::RetryInfo) + Send + Sync)>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>, krew_llm::LlmError> {
        let msg = self.error_msg.clone();
        Ok(Box::pin(stream::iter(vec![StreamEvent::Error(msg)])))
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn build_request(client: Arc<dyn LlmClient>) -> TaskRequest {
    TaskRequest {
        prompt: "hello".to_string(),
        system_prompt: None,
        client,
        tools: Arc::new(ToolRegistry::empty()),
        tool_defs: vec![],
        sampling: SamplingConfig::default(),
        max_rounds: 5,
        agent_name: "test-task".to_string(),
        approval_mode: ApprovalMode::FullAuto,
        approval_cache: ApprovalCache::new(),
        allow_rules: vec![],
        deny_rules: vec![],
        ask_rules: vec![],
        cwd: ".".to_string(),
    }
}

// ---------------------------------------------------------------------------
// run_task tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_task_basic_response() {
    let client: Arc<dyn LlmClient> = Arc::new(MockClient::new("hi there"));
    let result = run_task(build_request(client)).await;

    assert!(!result.is_error);
    assert_eq!(result.final_text, "hi there");
    assert!(result.usage.total_tokens > 0);

    // Last message should be assistant with correct name.
    let last = result.messages.last().unwrap();
    assert_eq!(last.role, ChatRole::Assistant);
    assert_eq!(last.content, "hi there");
    assert_eq!(last.name.as_deref(), Some("test-task"));
    assert!(last.usage.is_some());
}

#[tokio::test]
async fn run_task_with_system_prompt() {
    let client: Arc<dyn LlmClient> = Arc::new(MockClient::new("ok"));
    let mut req = build_request(client);
    req.system_prompt = Some("you are a helper".to_string());
    let result = run_task(req).await;

    assert!(!result.is_error);
    assert_eq!(result.final_text, "ok");
}

#[tokio::test]
async fn run_task_error_propagates() {
    let client: Arc<dyn LlmClient> = Arc::new(ErrorClient::new("something broke"));
    let result = run_task(build_request(client)).await;

    assert!(result.is_error);
    assert!(result.final_text.contains("something broke"));
}

#[tokio::test]
async fn run_task_messages_include_initial_and_final() {
    let client: Arc<dyn LlmClient> = Arc::new(MockClient::new("response"));
    let mut req = build_request(client);
    req.system_prompt = Some("sys prompt".to_string());
    let result = run_task(req).await;

    // Complete history: system + user + assistant
    assert!(result.messages.len() >= 3);
    assert_eq!(result.messages[0].role, ChatRole::System);
    assert_eq!(result.messages[0].content, "sys prompt");
    assert_eq!(result.messages[1].role, ChatRole::User);
    assert_eq!(result.messages[1].content, "hello");
    let last = result.messages.last().unwrap();
    assert_eq!(last.role, ChatRole::Assistant);
    assert_eq!(last.content, "response");
}

#[tokio::test]
async fn run_task_messages_without_system_prompt() {
    let client: Arc<dyn LlmClient> = Arc::new(MockClient::new("reply"));
    let result = run_task(build_request(client)).await;

    // No system prompt: user + assistant
    assert!(result.messages.len() >= 2);
    assert_eq!(result.messages[0].role, ChatRole::User);
    assert_eq!(result.messages[0].content, "hello");
    let last = result.messages.last().unwrap();
    assert_eq!(last.role, ChatRole::Assistant);
    assert_eq!(last.content, "reply");
}

// ---------------------------------------------------------------------------
// run_task_with_events tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn run_task_with_events_receives_done() {
    let client: Arc<dyn LlmClient> = Arc::new(MockClient::new("event test"));
    let (fut, mut rx) = run_task_with_events(build_request(client));

    // Spawn the task future.
    let result_handle = tokio::spawn(fut);

    // Collect events.
    let mut saw_text_delta = false;
    let mut saw_done = false;
    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::TextDelta(t) => {
                assert_eq!(t, "event test");
                saw_text_delta = true;
            }
            AgentEvent::Done { final_text, .. } => {
                assert_eq!(final_text, "event test");
                saw_done = true;
                break;
            }
            _ => {}
        }
    }

    assert!(saw_text_delta, "should receive TextDelta event");
    assert!(saw_done, "should receive Done event");

    let result = result_handle.await.unwrap();
    assert!(!result.is_error);
    assert_eq!(result.final_text, "event test");
}

#[tokio::test]
async fn run_task_with_events_receives_error() {
    let client: Arc<dyn LlmClient> = Arc::new(ErrorClient::new("boom"));
    let (fut, mut rx) = run_task_with_events(build_request(client));

    let result_handle = tokio::spawn(fut);

    let mut saw_error = false;
    while let Some(event) = rx.recv().await {
        if let AgentEvent::Error { message, .. } = event {
            assert!(message.contains("boom"));
            saw_error = true;
            break;
        }
    }

    assert!(saw_error, "should receive Error event");

    let result = result_handle.await.unwrap();
    assert!(result.is_error);
}
