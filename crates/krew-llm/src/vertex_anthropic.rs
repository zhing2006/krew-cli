//! Claude on Google Vertex AI implementation.
//!
//! Uses Vertex AI `:streamRawPredict` endpoints while reusing Anthropic
//! Messages request conversion and SSE stream parsing.

use crate::anthropic::{
    build_event_stream, build_output_config, build_sampling_params, build_thinking_params,
    convert_messages, convert_tools,
};
use crate::common::{self, AuthMode, RequestConfig};
use crate::{ChatMessage, LlmClient, LlmClientConfig, LlmError, StreamEvent, ToolDefinition};
use futures::Stream;
use krew_config::OtherAgentRole;
use krew_config::RetryConfig;
use krew_config::{SamplingConfig, ThinkingEffort};
use std::pin::Pin;

pub(crate) const VERTEX_ANTHROPIC_VERSION: &str = "vertex-2023-10-16";

/// Claude on Vertex AI client.
pub struct VertexAnthropicClient {
    http: reqwest::Client,
    base_url: Option<String>,
    api_key: String,
    model: String,
    agent_name: String,
    vertex_project: String,
    vertex_location: String,
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    enable_web_search: bool,
    other_agent_role: OtherAgentRole,
    retry_config: RetryConfig,
    extra_headers: Vec<(String, String)>,
}

impl VertexAnthropicClient {
    /// Create a new Vertex Anthropic client.
    pub fn new(
        config: LlmClientConfig,
        vertex_project: impl Into<String>,
        vertex_location: impl Into<String>,
    ) -> Self {
        let base_url = config
            .base_url
            .as_deref()
            .map(|url| url.trim_end_matches('/').to_string());

        Self {
            http: reqwest::Client::new(),
            base_url,
            api_key: config.api_key,
            model: config.model,
            agent_name: config.agent_name,
            vertex_project: vertex_project.into(),
            vertex_location: vertex_location.into(),
            enable_thinking: config.enable_thinking,
            thinking_effort: config.thinking_effort,
            enable_web_search: config.enable_web_search,
            other_agent_role: config.other_agent_role,
            retry_config: config.retry_config,
            extra_headers: config.extra_headers,
        }
    }
}

pub(crate) fn vertex_ai_host(location: &str) -> String {
    match location {
        "global" => "aiplatform.googleapis.com".to_string(),
        "us" => "aiplatform.us.rep.googleapis.com".to_string(),
        "eu" => "aiplatform.eu.rep.googleapis.com".to_string(),
        _ => format!("{location}-aiplatform.googleapis.com"),
    }
}

pub(crate) fn build_vertex_anthropic_predict_url(
    base_url: Option<&str>,
    project: &str,
    location: &str,
    model: &str,
) -> String {
    let suffix = format!(
        "/projects/{project}/locations/{location}/publishers/anthropic/models/{model}:streamRawPredict"
    );
    build_vertex_anthropic_url(base_url, location, &suffix)
}

pub(crate) fn build_vertex_anthropic_models_url(
    base_url: Option<&str>,
    project: &str,
    location: &str,
) -> String {
    let suffix = format!("/projects/{project}/locations/{location}/publishers/anthropic/models");
    build_vertex_anthropic_url(base_url, location, &suffix)
}

fn build_vertex_anthropic_url(base_url: Option<&str>, location: &str, suffix: &str) -> String {
    if let Some(base_url) = base_url {
        let base = base_url.trim_end_matches('/');
        if base.ends_with("/v1") {
            format!("{base}{suffix}")
        } else {
            format!("{base}/v1{suffix}")
        }
    } else {
        format!("https://{}/v1{suffix}", vertex_ai_host(location))
    }
}

#[async_trait::async_trait]
impl LlmClient for VertexAnthropicClient {
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        sampling: &SamplingConfig,
        on_retry: Option<&(dyn Fn(common::RetryInfo) + Send + Sync)>,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>, LlmError> {
        let url = build_vertex_anthropic_predict_url(
            self.base_url.as_deref(),
            &self.vertex_project,
            &self.vertex_location,
            &self.model,
        );

        let converted = convert_messages(messages, &self.agent_name, &self.other_agent_role);

        let mut body = serde_json::json!({
            "anthropic_version": VERTEX_ANTHROPIC_VERSION,
            "messages": converted.messages,
            "stream": true,
        });

        if let Some(system) = &converted.system {
            body["system"] = serde_json::json!(system);
        }

        let sampling_params = build_sampling_params(sampling, &self.model, self.enable_thinking);
        for (k, v) in sampling_params {
            body[k] = v;
        }

        if let Some(thinking) =
            build_thinking_params(self.enable_thinking, self.thinking_effort, &self.model)
        {
            body["thinking"] = thinking;
        }

        if let Some(output_config) =
            build_output_config(self.enable_thinking, self.thinking_effort, &self.model)
        {
            body["output_config"] = output_config;
        }

        if !tools.is_empty() || self.enable_web_search {
            let mut tool_list = convert_tools(tools);
            if self.enable_web_search {
                tool_list.push(serde_json::json!({
                    "type": "web_search_20250305",
                    "name": "web_search",
                }));
            }
            body["tools"] = serde_json::json!(tool_list);
        }

        let req_config = RequestConfig {
            http: &self.http,
            url: &url,
            body: &body,
            provider_name: "Vertex Anthropic",
        };
        let auth = AuthMode::Bearer(&self.api_key);
        let mut extra_headers = Vec::new();
        extra_headers.extend_from_slice(&self.extra_headers);
        let response = common::send_with_retry(
            &req_config,
            &auth,
            Some(&extra_headers),
            &self.retry_config,
            on_retry,
        )
        .await?;

        Ok(Box::pin(build_event_stream(response)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ChatRole, ThinkingBlock, ToolCallInfo, ToolDefinition};
    use futures::StreamExt;
    use krew_config::RetryConfig;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    struct CapturedRequest {
        request: String,
        body: serde_json::Value,
    }

    fn test_config(base_url: String, enable_web_search: bool) -> LlmClientConfig {
        LlmClientConfig {
            agent_name: "claude".into(),
            model: "claude-opus-4-7".into(),
            api_key: "ya29.token".into(),
            base_url: Some(base_url),
            other_agent_role: OtherAgentRole::User,
            retry_config: RetryConfig::default(),
            enable_thinking: false,
            thinking_effort: None,
            enable_web_search,
            extra_headers: vec![(
                "x-pass-anthropic-beta".into(),
                "context-1m-2025-08-07".into(),
            )],
        }
    }

    async fn run_capture_server(
        response_body: &'static str,
    ) -> (String, tokio::task::JoinHandle<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = Vec::new();
            let mut content_length;
            loop {
                let mut chunk = [0_u8; 1024];
                let n = socket.read(&mut chunk).await.unwrap();
                assert!(n > 0, "connection closed before headers");
                buffer.extend_from_slice(&chunk[..n]);
                let request = String::from_utf8_lossy(&buffer);
                if let Some(header_end) = request.find("\r\n\r\n") {
                    content_length = request[..header_end]
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap_or(0);
                    let body_len = buffer.len() - header_end - 4;
                    if body_len >= content_length {
                        break;
                    }
                }
            }

            let request = String::from_utf8_lossy(&buffer).to_string();
            let header_end = request.find("\r\n\r\n").unwrap();
            let body_start = header_end + 4;
            let body_bytes = &buffer[body_start..body_start + content_length];
            let body = serde_json::from_slice(body_bytes).unwrap();

            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            CapturedRequest { request, body }
        });
        (format!("http://{addr}/vertex_ai"), handle)
    }

    #[test]
    fn url_builder_google_vertex_locations() {
        assert_eq!(
            build_vertex_anthropic_predict_url(None, "my-project", "global", "claude-opus-4-7"),
            "https://aiplatform.googleapis.com/v1/projects/my-project/locations/global/publishers/anthropic/models/claude-opus-4-7:streamRawPredict"
        );
        assert_eq!(
            build_vertex_anthropic_predict_url(None, "my-project", "us", "claude-opus-4-7"),
            "https://aiplatform.us.rep.googleapis.com/v1/projects/my-project/locations/us/publishers/anthropic/models/claude-opus-4-7:streamRawPredict"
        );
        assert_eq!(
            build_vertex_anthropic_predict_url(None, "my-project", "eu", "claude-opus-4-7"),
            "https://aiplatform.eu.rep.googleapis.com/v1/projects/my-project/locations/eu/publishers/anthropic/models/claude-opus-4-7:streamRawPredict"
        );
        assert_eq!(
            build_vertex_anthropic_predict_url(None, "my-project", "us-east5", "claude-opus-4-7"),
            "https://us-east5-aiplatform.googleapis.com/v1/projects/my-project/locations/us-east5/publishers/anthropic/models/claude-opus-4-7:streamRawPredict"
        );
    }

    #[test]
    fn url_builder_passthrough_roots() {
        let suffix = "/projects/proj/locations/global/publishers/anthropic/models/claude-opus-4-7:streamRawPredict";
        assert_eq!(
            build_vertex_anthropic_predict_url(
                Some("https://litellm.example.com/vertex_ai"),
                "proj",
                "global",
                "claude-opus-4-7"
            ),
            format!("https://litellm.example.com/vertex_ai/v1{suffix}")
        );
        assert_eq!(
            build_vertex_anthropic_predict_url(
                Some("https://litellm.example.com/vertex_ai/"),
                "proj",
                "global",
                "claude-opus-4-7"
            ),
            format!("https://litellm.example.com/vertex_ai/v1{suffix}")
        );
        assert_eq!(
            build_vertex_anthropic_predict_url(
                Some("https://litellm.example.com/vertex_ai/v1"),
                "proj",
                "global",
                "claude-opus-4-7"
            ),
            format!("https://litellm.example.com/vertex_ai/v1{suffix}")
        );
        assert_eq!(
            build_vertex_anthropic_predict_url(
                Some("https://proxy.example.com"),
                "proj",
                "global",
                "claude-opus-4-7"
            ),
            format!("https://proxy.example.com/v1{suffix}")
        );
        assert_eq!(
            build_vertex_anthropic_predict_url(
                Some("https://proxy.example.com/api/v1/foo"),
                "proj",
                "global",
                "claude-opus-4-7"
            ),
            format!("https://proxy.example.com/api/v1/foo/v1{suffix}")
        );
        assert_eq!(
            build_vertex_anthropic_predict_url(
                Some("https://proxy.example.com/Vertex_AI"),
                "proj",
                "global",
                "claude-opus-4-7"
            ),
            format!("https://proxy.example.com/Vertex_AI/v1{suffix}")
        );
    }

    #[tokio::test]
    async fn chat_request_body_auth_headers_and_web_search() {
        let sse = "event: message_start\ndata: {\"message\":{\"usage\":{\"input_tokens\":5}}}\n\n\
                   event: message_delta\ndata: {\"usage\":{\"output_tokens\":7}}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let (base_url, handle) = run_capture_server(sse).await;
        let client = VertexAnthropicClient::new(test_config(base_url, true), "proj", "global");
        let messages = vec![ChatMessage::text(ChatRole::System, "be helpful", None)];
        let tools = vec![ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: serde_json::json!({"type": "object"}),
        }];

        let _stream = client
            .chat_stream(&messages, &tools, &SamplingConfig::default(), None)
            .await
            .unwrap();
        let captured = handle.await.unwrap();

        assert!(captured.request.starts_with(
            "POST /vertex_ai/v1/projects/proj/locations/global/publishers/anthropic/models/claude-opus-4-7:streamRawPredict "
        ));
        assert!(
            captured
                .request
                .contains("authorization: Bearer ya29.token")
        );
        assert!(
            captured
                .request
                .contains("x-pass-anthropic-beta: context-1m-2025-08-07")
        );
        assert_eq!(captured.body["anthropic_version"], VERTEX_ANTHROPIC_VERSION);
        assert_eq!(captured.body["stream"], true);
        assert_eq!(captured.body["system"], "be helpful");
        assert!(captured.body.get("max_tokens").is_some());
        assert!(captured.body.get("model").is_none());
        assert_eq!(captured.body["tools"][0]["name"], "read_file");
        assert_eq!(captured.body["tools"][1]["type"], "web_search_20250305");
        assert_eq!(captured.body["tools"][1]["name"], "web_search");
    }

    #[tokio::test]
    async fn passthrough_web_search_uses_anthropic_tool_type_and_stream_parser() {
        let sse = "event: content_block_start\ndata: {\"content_block\":{\"type\":\"server_tool_use\",\"name\":\"web_search\"}}\n\n\
                   event: content_block_delta\ndata: {\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"query\\\":\\\"rust\\\"}\"}}\n\n\
                   event: content_block_stop\ndata: {}\n\n\
                   event: message_delta\ndata: {\"usage\":{\"output_tokens\":3}}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let (base_url, handle) = run_capture_server(sse).await;
        let client = VertexAnthropicClient::new(test_config(base_url, true), "proj", "global");

        let mut stream = client
            .chat_stream(&[], &[], &SamplingConfig::default(), None)
            .await
            .unwrap();
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }

        let captured = handle.await.unwrap();
        assert_eq!(captured.body["tools"][0]["type"], "web_search_20250305");
        assert_ne!(captured.body["tools"][0]["type"], "web_search");
        assert!(matches!(
            events.first(),
            Some(StreamEvent::ServerToolStart { name }) if name == "web_search"
        ));
        assert!(events.iter().any(|event| matches!(
            event,
            StreamEvent::ServerToolDone {
                name,
                query: Some(query)
            } if name == "web_search" && query == "rust"
        )));
    }

    #[tokio::test]
    async fn web_search_disabled_does_not_inject_tool() {
        let sse = "event: message_stop\ndata: {}\n\n";
        let (base_url, handle) = run_capture_server(sse).await;
        let client = VertexAnthropicClient::new(test_config(base_url, false), "proj", "global");

        let _stream = client
            .chat_stream(&[], &[], &SamplingConfig::default(), None)
            .await
            .unwrap();
        let captured = handle.await.unwrap();
        assert!(captured.body.get("tools").is_none());
    }

    #[tokio::test]
    async fn replays_thinking_blocks_at_head_of_assistant_content() {
        let sse = "event: message_stop\ndata: {}\n\n";
        let (base_url, handle) = run_capture_server(sse).await;
        let client = VertexAnthropicClient::new(test_config(base_url, false), "proj", "global");

        let assistant = ChatMessage {
            role: ChatRole::Assistant,
            content: "calling tool".to_string(),
            name: Some("claude".to_string()),
            tool_calls: Some(vec![ToolCallInfo {
                id: "tool_1".to_string(),
                name: "read_file".to_string(),
                arguments: r#"{"path":"a"}"#.to_string(),
                thought_signature: None,
            }]),
            tool_call_id: None,
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: vec![ThinkingBlock::Thinking {
                text: "let me reason".to_string(),
                signature: "sig-vertex".to_string(),
            }],
            raw_content_blocks: Vec::new(),
        };

        let _stream = client
            .chat_stream(&[assistant], &[], &SamplingConfig::default(), None)
            .await
            .unwrap();
        let captured = handle.await.unwrap();

        let blocks = captured.body["messages"][0]["content"].as_array().unwrap();
        assert_eq!(blocks[0]["type"], "thinking");
        assert_eq!(blocks[0]["thinking"], "let me reason");
        assert_eq!(blocks[0]["signature"], "sig-vertex");
        assert!(
            blocks
                .iter()
                .any(|b| b.get("type").and_then(|t| t.as_str()) == Some("tool_use"))
        );
    }

    #[tokio::test]
    async fn parses_thinking_block_done_from_sse() {
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hi\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"vsig\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let (base_url, handle) = run_capture_server(sse).await;
        let client = VertexAnthropicClient::new(test_config(base_url, false), "proj", "global");

        let mut stream = client
            .chat_stream(&[], &[], &SamplingConfig::default(), None)
            .await
            .unwrap();
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        let _ = handle.await.unwrap();

        let block = events
            .iter()
            .find_map(|e| match e {
                StreamEvent::ThinkingBlockDone(b) => Some(b.clone()),
                _ => None,
            })
            .expect("Vertex SSE must emit ThinkingBlockDone");
        assert_eq!(
            block,
            ThinkingBlock::Thinking {
                text: "hi".to_string(),
                signature: "vsig".to_string(),
            }
        );
    }
}
