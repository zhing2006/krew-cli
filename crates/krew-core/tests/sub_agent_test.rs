use std::pin::Pin;
use std::sync::Arc;

use futures::stream;
use krew_config::{ApprovalMode, SamplingConfig};
use krew_llm::{ChatMessage, LlmClient, StreamEvent, ToolDefinition, Usage};
use krew_tools::{ToolContext, ToolHandler, ToolRegistry, ToolSpec};
use serde_json::json;

use krew_core::event::ApprovalCache;
use krew_core::sub_agent::run_agent_tool::PermissionConfig;
use krew_core::sub_agent::{RunAgentTool, SubAgentDef};

// ---------------------------------------------------------------------------
// Mock LlmClient
// ---------------------------------------------------------------------------

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
        _tools: &[ToolDefinition],
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

/// Mock client that captures the tool definitions it receives.
struct ToolCapturingClient {
    captured_tools: std::sync::Mutex<Option<Vec<String>>>,
}

impl ToolCapturingClient {
    fn new() -> Self {
        Self {
            captured_tools: std::sync::Mutex::new(None),
        }
    }

    fn captured_tool_names(&self) -> Vec<String> {
        self.captured_tools
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_default()
    }
}

#[async_trait::async_trait]
impl LlmClient for ToolCapturingClient {
    async fn chat_stream(
        &self,
        _messages: &[ChatMessage],
        tools: &[ToolDefinition],
        _sampling: &SamplingConfig,
        _on_retry: Option<&(dyn Fn(krew_llm::common::RetryInfo) + Send + Sync)>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>, krew_llm::LlmError> {
        let names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
        *self.captured_tools.lock().unwrap() = Some(names);
        let usage = Usage {
            prompt_tokens: 1,
            completion_tokens: 1,
            total_tokens: 2,
        };
        Ok(Box::pin(stream::iter(vec![
            StreamEvent::TextDelta("done".to_string()),
            StreamEvent::Done(usage),
        ])))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_sub_agent_def(name: &str) -> SubAgentDef {
    SubAgentDef {
        name: name.to_string(),
        description: "test agent".to_string(),
        system_prompt: "you are a test agent".to_string(),
        color: None,
        max_turns: 5,
        source_path: std::path::PathBuf::from("test.md"),
    }
}

fn make_perms() -> PermissionConfig {
    PermissionConfig {
        approval_mode: ApprovalMode::FullAuto,
        approval_cache: ApprovalCache::new(),
        allow_rules: vec![],
        deny_rules: vec![],
        ask_rules: vec![],
        cwd: ".".to_string(),
    }
}

fn make_tool_context(registry: Arc<ToolRegistry>) -> ToolContext {
    ToolContext {
        output_tx: None,
        parent_event_tx: None,
        tool_registry: Some(Box::new(registry)),
    }
}

fn dummy_tool_spec(name: &str) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: format!("{name} tool"),
        parameters: json!({"type": "object", "properties": {}}),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn depth_guard_prevents_nesting() {
    let client: Arc<dyn LlmClient> = Arc::new(MockClient::new("hello"));
    let tool = RunAgentTool::new(
        vec![make_sub_agent_def("helper")],
        client,
        SamplingConfig::default(),
        make_perms(),
    );

    // Simulate already running by setting the flag.
    tool.set_running_for_test(true);

    let registry = Arc::new(ToolRegistry::empty());
    let ctx = make_tool_context(registry);
    let args = json!({"agent": "helper", "task": "do something"});

    let result = tool.execute(args, &ctx).await.unwrap();
    assert!(result.is_error);
    assert!(result.content.contains("nesting is not allowed"));
}

#[tokio::test]
async fn run_agent_excluded_from_tool_defs() {
    let capturing_client = Arc::new(ToolCapturingClient::new());
    let client: Arc<dyn LlmClient> = capturing_client.clone();
    let tool = RunAgentTool::new(
        vec![make_sub_agent_def("helper")],
        client,
        SamplingConfig::default(),
        make_perms(),
    );

    // Registry contains run_agent + other tools.
    let mut registry = ToolRegistry::empty();
    registry.register(dummy_tool_spec("read_file"), Box::new(NoopHandler));
    registry.register(dummy_tool_spec("run_agent"), Box::new(NoopHandler));
    registry.register(dummy_tool_spec("grep"), Box::new(NoopHandler));
    let registry = Arc::new(registry);

    let ctx = make_tool_context(registry);
    let args = json!({"agent": "helper", "task": "find files"});

    let result = tool.execute(args, &ctx).await.unwrap();
    assert!(!result.is_error);

    // The mock client should have received tools WITHOUT run_agent.
    let captured = capturing_client.captured_tool_names();
    assert!(captured.contains(&"read_file".to_string()));
    assert!(captured.contains(&"grep".to_string()));
    assert!(
        !captured.contains(&"run_agent".to_string()),
        "run_agent should be excluded from tool_defs"
    );
}

/// Minimal no-op tool handler for registry construction.
struct NoopHandler;

#[async_trait::async_trait]
impl krew_tools::ToolHandler for NoopHandler {
    fn name(&self) -> &str {
        "noop"
    }
    fn requires_approval(&self) -> bool {
        false
    }
    async fn execute(
        &self,
        _args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<krew_tools::ToolResult, krew_tools::ToolError> {
        Ok(krew_tools::ToolResult {
            content: String::new(),
            is_error: false,
            images: vec![],
        })
    }
}
