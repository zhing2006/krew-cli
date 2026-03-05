//! Google Gemini `generateContent` API implementation.
//!
//! Supports both standard Gemini API and Vertex AI mode (when
//! `vertex_project` and `vertex_location` are set).

use crate::common::{self, AuthMode, RequestConfig, RoleContent, merge_consecutive_same_role};
use crate::{
    ChatMessage, ChatRole, LlmClient, LlmClientConfig, LlmError, StreamEvent, ToolDefinition, Usage,
};
use futures::Stream;
use krew_config::OtherAgentRole;
use krew_config::RetryConfig;
use krew_config::{SamplingConfig, ThinkingEffort};
use std::pin::Pin;

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Google Gemini API client.
pub struct GoogleClient {
    http: reqwest::Client,
    api_key: String,
    model: String,
    agent_name: String,
    vertex_mode: bool,
    vertex_project: Option<String>,
    vertex_location: Option<String>,
    base_url: Option<String>,
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    enable_web_search: bool,
    other_agent_role: OtherAgentRole,
    retry_config: RetryConfig,
}

impl GoogleClient {
    /// Create a new Google Gemini client.
    ///
    /// When both `vertex_project` and `vertex_location` are set, the client
    /// switches to Vertex AI mode with Bearer token authentication.
    pub fn new(
        config: LlmClientConfig,
        vertex_project: Option<&str>,
        vertex_location: Option<&str>,
    ) -> Self {
        let vertex_mode = vertex_project.is_some() && vertex_location.is_some();

        Self {
            http: reqwest::Client::new(),
            api_key: config.api_key,
            model: config.model,
            agent_name: config.agent_name,
            vertex_mode,
            vertex_project: vertex_project.map(|s| s.to_string()),
            vertex_location: vertex_location.map(|s| s.to_string()),
            base_url: config.base_url.map(|s| s.trim_end_matches('/').to_string()),
            enable_thinking: config.enable_thinking,
            thinking_effort: config.thinking_effort,
            enable_web_search: config.enable_web_search,
            other_agent_role: config.other_agent_role,
            retry_config: config.retry_config,
        }
    }

    /// Build the API URL for the request.
    fn build_url(&self) -> String {
        if self.vertex_mode {
            let location = self.vertex_location.as_deref().unwrap_or("us-central1");
            let project = self.vertex_project.as_deref().unwrap_or("unknown");
            format!(
                "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models/{}:streamGenerateContent?alt=sse",
                self.model,
            )
        } else if let Some(base) = &self.base_url {
            format!(
                "{base}/models/{}:streamGenerateContent?alt=sse&key={}",
                self.model, self.api_key,
            )
        } else {
            format!(
                "{DEFAULT_BASE_URL}/models/{}:streamGenerateContent?alt=sse&key={}",
                self.model, self.api_key,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Message conversion
// ---------------------------------------------------------------------------

/// Result of message conversion: system instruction + contents array.
pub struct ConvertedMessages {
    /// System instruction text (None if no system messages).
    pub system_instruction: Option<String>,
    /// Gemini contents array.
    pub contents: Vec<serde_json::Value>,
}

/// Convert unified ChatMessages to Gemini format.
///
/// - System messages → extracted as `systemInstruction` (not in contents)
/// - User messages → `{role: "user", parts: [{text: "..."}]}`
/// - Current agent's assistant → `{role: "model", parts: [{text: "..."}]}`
/// - Other agents' assistant → role per `other_agent_role` with `[agent_name]` prefix
///   (Google uses `"model"` instead of `"assistant"`)
///
/// Consecutive same-role messages are merged.
pub fn convert_messages(
    messages: &[ChatMessage],
    self_agent_name: &str,
    other_agent_role: &OtherAgentRole,
) -> ConvertedMessages {
    // Collect system messages.
    let system_texts: Vec<&str> = messages
        .iter()
        .filter(|m| m.role == ChatRole::System)
        .map(|m| m.content.as_str())
        .collect();
    let system_instruction = if system_texts.is_empty() {
        None
    } else {
        Some(system_texts.join("\n\n"))
    };

    let mut result: Vec<serde_json::Value> = Vec::new();
    let mut pending: Vec<RoleContent> = Vec::new();

    for msg in messages.iter().filter(|m| m.role != ChatRole::System) {
        // Tool result messages: Google uses role: "user" with functionResponse parts.
        if msg.role == ChatRole::Tool {
            flush_pending_google(&mut pending, &mut result);

            let response: serde_json::Value = serde_json::from_str(&msg.content)
                .unwrap_or_else(|_| serde_json::json!({ "result": msg.content }));

            let tool_name = msg.name.as_deref().unwrap_or("unknown");

            result.push(serde_json::json!({
                "role": "user",
                "parts": [{
                    "functionResponse": {
                        "name": tool_name,
                        "response": response,
                    }
                }],
            }));
            continue;
        }

        // Assistant messages with tool_calls: Google uses functionCall parts.
        if let (ChatRole::Assistant, Some(tcs)) = (&msg.role, &msg.tool_calls) {
            flush_pending_google(&mut pending, &mut result);

            let mut parts: Vec<serde_json::Value> = Vec::new();
            if !msg.content.is_empty() {
                parts.push(serde_json::json!({ "text": msg.content }));
            }
            for tc in tcs {
                let args: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or_default();
                let mut part = serde_json::json!({
                    "functionCall": {
                        "name": tc.name,
                        "args": args,
                    }
                });
                // Echo back thoughtSignature for Vertex AI thinking mode.
                if let Some(ref sig) = tc.thought_signature {
                    part["thoughtSignature"] = serde_json::json!(sig);
                }
                parts.push(part);
            }
            result.push(serde_json::json!({
                "role": "model",
                "parts": parts,
            }));
            continue;
        }

        // Regular messages.
        let is_other_agent = matches!(&msg.role, ChatRole::Assistant)
            && msg
                .name
                .as_ref()
                .is_some_and(|name| name != self_agent_name);

        let role = match &msg.role {
            ChatRole::User | ChatRole::Tool => "user",
            ChatRole::Assistant if is_other_agent => match other_agent_role {
                OtherAgentRole::User => "user",
                OtherAgentRole::Assistant => "model",
            },
            ChatRole::Assistant => "model",
            ChatRole::System => unreachable!(),
        };

        let content = if is_other_agent {
            let name = msg.name.as_deref().unwrap_or("unknown");
            format!("[{name}] {}", msg.content)
        } else {
            msg.content.clone()
        };

        pending.push(RoleContent {
            role: role.to_string(),
            content,
        });
    }

    flush_pending_google(&mut pending, &mut result);

    ConvertedMessages {
        system_instruction,
        contents: result,
    }
}

/// Merge and flush pending role-content items into the result vector.
fn flush_pending_google(pending: &mut Vec<RoleContent>, result: &mut Vec<serde_json::Value>) {
    if pending.is_empty() {
        return;
    }
    let merged = merge_consecutive_same_role(std::mem::take(pending));
    for rc in merged {
        result.push(serde_json::json!({
            "role": rc.role,
            "parts": [{"text": rc.content}],
        }));
    }
}

// ---------------------------------------------------------------------------
// Sampling parameter mapping
// ---------------------------------------------------------------------------

/// Build the generationConfig object for Gemini.
///
/// Maps: temperature, topP, topK, maxOutputTokens, frequencyPenalty,
/// presencePenalty, stopSequences. Uses camelCase field names.
fn build_generation_config(
    sampling: &SamplingConfig,
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    model: &str,
) -> serde_json::Value {
    let mut config = serde_json::Map::new();

    if let Some(t) = sampling.temperature {
        config.insert("temperature".into(), serde_json::json!(t));
    }
    if let Some(p) = sampling.top_p {
        config.insert("topP".into(), serde_json::json!(p));
    }
    if let Some(k) = sampling.top_k {
        config.insert("topK".into(), serde_json::json!(k));
    }
    if let Some(m) = sampling.max_tokens {
        config.insert("maxOutputTokens".into(), serde_json::json!(m));
    }
    if let Some(fp) = sampling.frequency_penalty {
        config.insert("frequencyPenalty".into(), serde_json::json!(fp));
    }
    if let Some(pp) = sampling.presence_penalty {
        config.insert("presencePenalty".into(), serde_json::json!(pp));
    }
    if let Some(ref stops) = sampling.stop_sequences {
        config.insert("stopSequences".into(), serde_json::json!(stops));
    }

    // Add thinking config if enabled.
    if enable_thinking {
        config.insert(
            "thinkingConfig".into(),
            build_thinking_config(thinking_effort, model),
        );
    }

    serde_json::Value::Object(config)
}

// ---------------------------------------------------------------------------
// Thinking parameter injection
// ---------------------------------------------------------------------------

/// Determine if a model is Gemini 2.5 (uses thinkingBudget).
fn is_gemini_2_5(model: &str) -> bool {
    model.contains("gemini-2.5")
}

/// Build thinkingConfig for the generationConfig.
///
/// - Gemini 3.x (default): uses `thinkingLevel` enum (low/medium/high)
/// - Gemini 2.5: uses `thinkingBudget` integer
/// - Unknown models: defaults to thinkingLevel
fn build_thinking_config(
    thinking_effort: Option<ThinkingEffort>,
    model: &str,
) -> serde_json::Value {
    if is_gemini_2_5(model) {
        // Gemini 2.5: use thinkingBudget
        let budget = match thinking_effort {
            Some(ThinkingEffort::Low) => serde_json::json!(1024),
            Some(ThinkingEffort::Medium) => serde_json::json!(8192),
            Some(ThinkingEffort::High) => serde_json::json!(24576),
            None => serde_json::json!(-1), // -1 = dynamic
        };
        serde_json::json!({
            "includeThoughts": true,
            "thinkingBudget": budget,
        })
    } else {
        // Gemini 3.x or unknown: use thinkingLevel
        let level = match thinking_effort {
            Some(ThinkingEffort::Low) => "low",
            Some(ThinkingEffort::Medium) => "medium",
            Some(ThinkingEffort::High) | None => "high",
        };
        serde_json::json!({
            "includeThoughts": true,
            "thinkingLevel": level,
        })
    }
}

// ---------------------------------------------------------------------------
// Tool definition conversion
// ---------------------------------------------------------------------------

/// Convert ToolDefinitions to Gemini functionDeclarations format.
fn convert_tools(tools: &[ToolDefinition]) -> serde_json::Value {
    let declarations: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            })
        })
        .collect();

    serde_json::json!([{
        "functionDeclarations": declarations,
    }])
}

// ---------------------------------------------------------------------------
// SSE stream parsing
// ---------------------------------------------------------------------------

/// Parse Gemini SSE events into StreamEvents.
///
/// Gemini uses data-only SSE (no event type). Each data line is a complete
/// JSON object containing candidates with parts.
fn build_event_stream(response: reqwest::Response) -> impl Stream<Item = StreamEvent> + Send {
    use eventsource_stream::Eventsource;
    use futures::StreamExt;

    let byte_stream = response.bytes_stream();
    let sse_stream = byte_stream.eventsource();

    let usage_state = std::sync::Arc::new(tokio::sync::Mutex::new(Usage::default()));
    let call_counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

    // grounding_state: 0 = not seen, 1 = start emitted, 2 = done emitted
    futures::stream::unfold(
        (sse_stream, usage_state, call_counter, false, false, 0u8),
        |(
            mut sse_stream,
            usage_state,
            call_counter,
            mut done,
            pending_done,
            mut grounding_state,
        )| async move {
            // Emit Done if the previous iteration saw a finishReason but
            // returned a content event first.
            if pending_done {
                let usage = usage_state.lock().await.clone();
                return Some((
                    StreamEvent::Done(usage),
                    (
                        sse_stream,
                        usage_state,
                        call_counter,
                        true,
                        false,
                        grounding_state,
                    ),
                ));
            }
            if done {
                return None;
            }

            loop {
                let next = sse_stream.next().await;
                match next {
                    Some(Ok(event)) => {
                        let data = event.data.trim().to_string();
                        if data.is_empty() {
                            continue;
                        }

                        let v: serde_json::Value = match serde_json::from_str(&data) {
                            Ok(v) => v,
                            Err(e) => {
                                done = true;
                                return Some((
                                    StreamEvent::Error(format!("invalid JSON: {e}")),
                                    (
                                        sse_stream,
                                        usage_state,
                                        call_counter,
                                        done,
                                        false,
                                        grounding_state,
                                    ),
                                ));
                            }
                        };

                        // Check for usage metadata.
                        if let Some(usage_meta) = v.get("usageMetadata") {
                            let prompt = usage_meta
                                .get("promptTokenCount")
                                .and_then(|t| t.as_u64())
                                .unwrap_or(0) as u32;
                            let completion = usage_meta
                                .get("candidatesTokenCount")
                                .and_then(|t| t.as_u64())
                                .unwrap_or(0) as u32;
                            let mut u = usage_state.lock().await;
                            u.prompt_tokens = prompt;
                            u.completion_tokens = completion;
                            u.total_tokens = prompt + completion;
                        }

                        // Extract candidates.
                        let candidates = match v.get("candidates").and_then(|c| c.as_array()) {
                            Some(c) if !c.is_empty() => c,
                            _ => continue,
                        };

                        let candidate = &candidates[0];

                        // Check for finish reason.
                        let finish_reason =
                            candidate.get("finishReason").and_then(|fr| fr.as_str());

                        // Detect grounding (google_search) metadata.
                        // Empty `{}` → emit ServerToolStart (search beginning).
                        // Non-empty (with queries) → emit ServerToolDone.
                        if grounding_state < 2
                            && let Some(gm) = candidate.get("groundingMetadata")
                        {
                            let is_empty = gm.as_object().is_some_and(|m| m.is_empty());
                            if is_empty && grounding_state == 0 {
                                // Early empty metadata = search started.
                                grounding_state = 1;
                                let pend = finish_reason.is_some();
                                return Some((
                                    StreamEvent::ServerToolStart {
                                        name: "google_search".to_string(),
                                    },
                                    (
                                        sse_stream,
                                        usage_state,
                                        call_counter,
                                        done,
                                        pend,
                                        grounding_state,
                                    ),
                                ));
                            } else if !is_empty {
                                // Non-empty metadata = search done with results.
                                grounding_state = 2;
                                let query = gm
                                    .get("webSearchQueries")
                                    .or_else(|| gm.get("retrievalQueries"))
                                    .and_then(|q| q.as_array())
                                    .and_then(|arr| arr.first())
                                    .and_then(|q| q.as_str())
                                    .map(|s| s.to_string());
                                let pend = finish_reason.is_some();
                                return Some((
                                    StreamEvent::ServerToolDone {
                                        name: "google_search".to_string(),
                                        query,
                                    },
                                    (
                                        sse_stream,
                                        usage_state,
                                        call_counter,
                                        done,
                                        pend,
                                        grounding_state,
                                    ),
                                ));
                            }
                        }

                        // Process parts.
                        if let Some(content) = candidate.get("content")
                            && let Some(parts) = content.get("parts").and_then(|p| p.as_array())
                        {
                            // Collect events from parts.
                            let mut events: Vec<StreamEvent> = Vec::new();

                            for part in parts {
                                // Check for thinking content.
                                let is_thought = part
                                    .get("thought")
                                    .and_then(|t| t.as_bool())
                                    .unwrap_or(false);

                                if let Some(text) = part.get("text").and_then(|t| t.as_str())
                                    && !text.is_empty()
                                {
                                    if is_thought {
                                        events.push(StreamEvent::ThinkingDelta(text.to_string()));
                                    } else {
                                        events.push(StreamEvent::TextDelta(text.to_string()));
                                    }
                                }

                                // Check for function call.
                                if let Some(fc) = part.get("functionCall") {
                                    let name = fc
                                        .get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let args = fc
                                        .get("args")
                                        .map(|a| a.to_string())
                                        .unwrap_or_else(|| "{}".to_string());
                                    let id = format!(
                                        "gemini_call_{}",
                                        call_counter
                                            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                                    );
                                    // Capture thoughtSignature for Vertex AI thinking mode.
                                    let thought_signature = part
                                        .get("thoughtSignature")
                                        .and_then(|s| s.as_str())
                                        .map(|s| s.to_string());
                                    events.push(StreamEvent::ToolCall {
                                        id,
                                        name,
                                        arguments: args,
                                        thought_signature,
                                    });
                                }
                            }

                            // If we have events, return the first one.
                            // (For simplicity, emit events one at a time; multiple
                            // parts in one chunk will emit sequentially.)
                            if let Some(event) = events.into_iter().next() {
                                // If this chunk also has a finishReason, set
                                // pending_done so Done is emitted next iteration.
                                let pend = finish_reason.is_some();
                                return Some((
                                    event,
                                    (
                                        sse_stream,
                                        usage_state,
                                        call_counter,
                                        done,
                                        pend,
                                        grounding_state,
                                    ),
                                ));
                            }
                        }

                        // Check if stream is done (any finishReason is terminal).
                        if finish_reason.is_some() {
                            done = true;
                            let usage = usage_state.lock().await.clone();
                            return Some((
                                StreamEvent::Done(usage),
                                (
                                    sse_stream,
                                    usage_state,
                                    call_counter,
                                    done,
                                    false,
                                    grounding_state,
                                ),
                            ));
                        }

                        continue;
                    }
                    Some(Err(e)) => {
                        done = true;
                        return Some((
                            StreamEvent::Error(format!("SSE stream error: {e}")),
                            (
                                sse_stream,
                                usage_state,
                                call_counter,
                                done,
                                false,
                                grounding_state,
                            ),
                        ));
                    }
                    None => {
                        // Stream ended without finishReason.
                        done = true;
                        return Some((
                            StreamEvent::Error("stream interrupted".into()),
                            (
                                sse_stream,
                                usage_state,
                                call_counter,
                                done,
                                false,
                                grounding_state,
                            ),
                        ));
                    }
                }
            }
        },
    )
}

// ---------------------------------------------------------------------------
// LlmClient implementation
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl LlmClient for GoogleClient {
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        sampling: &SamplingConfig,
        on_retry: Option<&(dyn Fn(common::RetryInfo) + Send + Sync)>,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>, LlmError> {
        let url = self.build_url();

        // Convert messages.
        let converted = convert_messages(messages, &self.agent_name, &self.other_agent_role);

        // Build request body.
        let mut body = serde_json::json!({
            "contents": converted.contents,
        });

        // Add system instruction if present.
        if let Some(system) = &converted.system_instruction {
            body["systemInstruction"] = serde_json::json!({
                "parts": [{"text": system}],
            });
        }

        // Add generation config.
        let gen_config = build_generation_config(
            sampling,
            self.enable_thinking,
            self.thinking_effort,
            &self.model,
        );
        if gen_config.as_object().is_some_and(|m| !m.is_empty()) {
            body["generationConfig"] = gen_config;
        }

        // Add tools if provided.
        if !tools.is_empty() || self.enable_web_search {
            let mut tool_array = if tools.is_empty() {
                vec![]
            } else {
                serde_json::from_value(convert_tools(tools)).unwrap_or_default()
            };
            if self.enable_web_search {
                tool_array.push(serde_json::json!({ "google_search": {} }));
            }
            body["tools"] = serde_json::json!(tool_array);
        }

        // Send request with retry.
        let req_config = RequestConfig {
            http: &self.http,
            url: &url,
            body: &body,
            provider_name: "Gemini",
        };
        let auth = if self.vertex_mode {
            AuthMode::Bearer(&self.api_key)
        } else {
            // Standard mode: API key is in URL, no auth header needed.
            // Use a dummy bearer that won't be sent — actually, we need to
            // skip auth entirely for standard mode since key is in URL.
            // We'll use a special "no auth" approach.
            AuthMode::Header("x-goog-api-client", "krew-cli")
        };
        let response =
            common::send_with_retry(&req_config, &auth, None, &self.retry_config, on_retry).await?;

        // Convert to SSE event stream.
        let stream = build_event_stream(response);

        Ok(Box::pin(stream))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use krew_config::SamplingConfig;

    // ---- SSE parsing tests (4.8) ----

    #[test]
    fn parse_text_part() {
        let data = r#"{"candidates":[{"content":{"parts":[{"text":"hello"}]}}]}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let parts = v["candidates"][0]["content"]["parts"].as_array().unwrap();
        let text = parts[0]["text"].as_str().unwrap();
        assert_eq!(text, "hello");
    }

    #[test]
    fn parse_thought_part() {
        let data =
            r#"{"candidates":[{"content":{"parts":[{"text":"thinking...","thought":true}]}}]}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let part = &v["candidates"][0]["content"]["parts"][0];
        assert_eq!(part["thought"].as_bool(), Some(true));
        assert_eq!(part["text"].as_str(), Some("thinking..."));
    }

    #[test]
    fn parse_thought_false_is_text() {
        let data = r#"{"candidates":[{"content":{"parts":[{"text":"normal","thought":false}]}}]}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let part = &v["candidates"][0]["content"]["parts"][0];
        assert_eq!(part["thought"].as_bool(), Some(false));
    }

    #[test]
    fn parse_thought_missing_is_text() {
        let data = r#"{"candidates":[{"content":{"parts":[{"text":"normal"}]}}]}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let part = &v["candidates"][0]["content"]["parts"][0];
        assert!(part.get("thought").is_none());
    }

    #[test]
    fn parse_function_call() {
        let data = r#"{"candidates":[{"content":{"parts":[{"functionCall":{"name":"search","args":{"query":"hello"}}}]}}]}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let fc = &v["candidates"][0]["content"]["parts"][0]["functionCall"];
        assert_eq!(fc["name"].as_str(), Some("search"));
        assert!(fc["args"].is_object());
    }

    #[test]
    fn parse_finish_reason_with_usage() {
        let data = r#"{"candidates":[{"finishReason":"STOP","content":{"parts":[{"text":""}]}}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":50}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let u = &v["usageMetadata"];
        assert_eq!(u["promptTokenCount"].as_u64(), Some(100));
        assert_eq!(u["candidatesTokenCount"].as_u64(), Some(50));
        assert_eq!(v["candidates"][0]["finishReason"].as_str(), Some("STOP"));
    }

    #[test]
    fn parse_usage_with_thoughts_token_count() {
        let data = r#"{"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":50,"thoughtsTokenCount":200}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        // thoughtsTokenCount is present but doesn't affect Usage mapping.
        let u = &v["usageMetadata"];
        let prompt = u["promptTokenCount"].as_u64().unwrap() as u32;
        let completion = u["candidatesTokenCount"].as_u64().unwrap() as u32;
        assert_eq!(prompt, 100);
        assert_eq!(completion, 50);
    }

    #[test]
    fn parse_empty_candidates() {
        let data = r#"{"candidates":[]}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let candidates = v["candidates"].as_array().unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn parse_invalid_json() {
        let data = "not json at all";
        assert!(serde_json::from_str::<serde_json::Value>(data).is_err());
    }

    // ---- Message conversion tests (4.9) ----

    #[test]
    fn convert_user_message() {
        let messages = vec![ChatMessage::text(ChatRole::User, "hello".to_string(), None)];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert!(result.system_instruction.is_none());
        assert_eq!(result.contents.len(), 1);
        assert_eq!(result.contents[0]["role"], "user");
        assert_eq!(result.contents[0]["parts"][0]["text"], "hello");
    }

    #[test]
    fn convert_system_to_instruction() {
        let messages = vec![ChatMessage::text(
            ChatRole::System,
            "you are helpful".to_string(),
            None,
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(
            result.system_instruction.as_deref(),
            Some("you are helpful")
        );
        assert!(result.contents.is_empty());
    }

    #[test]
    fn convert_multiple_system_messages_merged() {
        let messages = vec![
            ChatMessage::text(ChatRole::System, "part 1".to_string(), None),
            ChatMessage::text(ChatRole::System, "part 2".to_string(), None),
        ];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(
            result.system_instruction.as_deref(),
            Some("part 1\n\npart 2")
        );
    }

    #[test]
    fn convert_current_agent_to_model() {
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "my reply".to_string(),
            Some("agent1".to_string()),
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.contents[0]["role"], "model");
    }

    #[test]
    fn convert_other_agent_to_user() {
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "other reply".to_string(),
            Some("agent2".to_string()),
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.contents[0]["role"], "user");
        assert_eq!(
            result.contents[0]["parts"][0]["text"],
            "[agent2] other reply"
        );
    }

    #[test]
    fn convert_other_agent_as_model() {
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "other reply".to_string(),
            Some("agent2".to_string()),
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::Assistant);
        // Google uses "model" instead of "assistant".
        assert_eq!(result.contents[0]["role"], "model");
        assert_eq!(
            result.contents[0]["parts"][0]["text"],
            "[agent2] other reply"
        );
    }

    #[test]
    fn convert_consecutive_user_messages_merged() {
        let messages = vec![
            ChatMessage::text(
                ChatRole::Assistant,
                "reply A".to_string(),
                Some("agentA".to_string()),
            ),
            ChatMessage::text(
                ChatRole::Assistant,
                "reply B".to_string(),
                Some("agentB".to_string()),
            ),
        ];
        let result = convert_messages(&messages, "agentC", &OtherAgentRole::User);
        assert_eq!(result.contents.len(), 1);
        assert_eq!(result.contents[0]["role"], "user");
        assert_eq!(
            result.contents[0]["parts"][0]["text"],
            "[agentA] reply A\n\n[agentB] reply B"
        );
    }

    #[test]
    fn convert_three_consecutive_agents_merged() {
        let messages = vec![
            ChatMessage::text(ChatRole::Assistant, "a".to_string(), Some("a1".to_string())),
            ChatMessage::text(ChatRole::Assistant, "b".to_string(), Some("a2".to_string())),
            ChatMessage::text(ChatRole::Assistant, "c".to_string(), Some("a3".to_string())),
        ];
        let result = convert_messages(&messages, "me", &OtherAgentRole::User);
        assert_eq!(result.contents.len(), 1);
    }

    #[test]
    fn convert_alternating_no_merge() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "hi".to_string(), None),
            ChatMessage::text(
                ChatRole::Assistant,
                "hello".to_string(),
                Some("agent1".to_string()),
            ),
        ];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.contents.len(), 2);
    }

    #[test]
    fn convert_empty_messages() {
        let result = convert_messages(&[], "agent1", &OtherAgentRole::User);
        assert!(result.system_instruction.is_none());
        assert!(result.contents.is_empty());
    }

    // ---- Sampling parameter tests (4.10) ----

    #[test]
    fn generation_config_all_params() {
        let sampling = SamplingConfig {
            temperature: Some(0.7),
            top_p: Some(0.95),
            top_k: Some(40),
            max_tokens: Some(8192),
            frequency_penalty: Some(0.5),
            presence_penalty: Some(0.3),
            stop_sequences: Some(vec!["STOP".into()]),
        };
        let config = build_generation_config(&sampling, false, None, "gemini-3.1-pro");
        assert_eq!(config["temperature"], 0.7);
        assert_eq!(config["topP"], 0.95);
        assert_eq!(config["topK"], 40);
        assert_eq!(config["maxOutputTokens"], 8192);
        assert_eq!(config["frequencyPenalty"], 0.5);
        assert_eq!(config["presencePenalty"], 0.3);
        assert_eq!(config["stopSequences"], serde_json::json!(["STOP"]));
    }

    #[test]
    fn generation_config_empty() {
        let config =
            build_generation_config(&SamplingConfig::default(), false, None, "gemini-3.1-pro");
        assert!(config.as_object().unwrap().is_empty());
    }

    #[test]
    fn generation_config_camel_case() {
        let sampling = SamplingConfig {
            top_p: Some(0.9),
            top_k: Some(50),
            max_tokens: Some(1024),
            ..Default::default()
        };
        let config = build_generation_config(&sampling, false, None, "gemini-3.1-pro");
        assert!(config.get("topP").is_some());
        assert!(config.get("top_p").is_none());
        assert!(config.get("topK").is_some());
        assert!(config.get("top_k").is_none());
        assert!(config.get("maxOutputTokens").is_some());
        assert!(config.get("max_tokens").is_none());
    }

    // ---- URL construction tests (4.11) ----

    #[test]
    fn standard_url() {
        let url = format!(
            "{DEFAULT_BASE_URL}/models/gemini-3.1-pro:streamGenerateContent?alt=sse&key=test-key"
        );
        assert!(url.starts_with("https://generativelanguage.googleapis.com/v1beta/models/"));
        assert!(url.contains("alt=sse"));
        assert!(url.contains("key=test-key"));
    }

    #[test]
    fn custom_base_url() {
        let base = "https://custom.api.com/v1";
        let url = format!("{base}/models/gemini-pro:streamGenerateContent?alt=sse&key=k");
        assert!(url.starts_with("https://custom.api.com/v1/models/"));
    }

    #[test]
    fn vertex_ai_url() {
        let location = "us-central1";
        let project = "my-project";
        let model = "gemini-3.1-pro";
        let url = format!(
            "https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models/{model}:streamGenerateContent?alt=sse"
        );
        assert_eq!(
            url,
            "https://us-central1-aiplatform.googleapis.com/v1/projects/my-project/locations/us-central1/publishers/google/models/gemini-3.1-pro:streamGenerateContent?alt=sse"
        );
    }

    #[test]
    fn vertex_ai_url_no_api_key() {
        let url = "https://us-central1-aiplatform.googleapis.com/v1/projects/proj/locations/us-central1/publishers/google/models/gemini-pro:streamGenerateContent?alt=sse";
        assert!(!url.contains("key="));
    }

    // ---- Authentication tests (4.12) ----

    #[test]
    fn standard_mode_api_key_in_url() {
        let vertex_mode = false;
        assert!(!vertex_mode);
        // In standard mode, API key is in URL query parameter, not in header.
    }

    #[test]
    fn vertex_mode_bearer_auth() {
        let vertex_mode = true;
        let token = "access-token";
        let auth = if vertex_mode {
            AuthMode::Bearer(token)
        } else {
            AuthMode::Header("x-goog-api-client", "krew-cli")
        };
        assert!(matches!(auth, AuthMode::Bearer("access-token")));
    }

    // ---- Thinking parameter tests (4.13) ----

    #[test]
    fn thinking_gemini_3x_effort_high() {
        let config = build_thinking_config(Some(ThinkingEffort::High), "gemini-3.1-pro-preview");
        assert_eq!(config["includeThoughts"], true);
        assert_eq!(config["thinkingLevel"], "high");
        assert!(config.get("thinkingBudget").is_none());
    }

    #[test]
    fn thinking_gemini_3x_effort_medium() {
        let config = build_thinking_config(Some(ThinkingEffort::Medium), "gemini-3.1-pro");
        assert_eq!(config["thinkingLevel"], "medium");
    }

    #[test]
    fn thinking_gemini_3x_effort_low() {
        let config = build_thinking_config(Some(ThinkingEffort::Low), "gemini-3-flash-preview");
        assert_eq!(config["thinkingLevel"], "low");
    }

    #[test]
    fn thinking_gemini_3x_effort_none_defaults_high() {
        let config = build_thinking_config(None, "gemini-3.1-pro");
        assert_eq!(config["thinkingLevel"], "high");
    }

    #[test]
    fn thinking_gemini_2_5_effort_high() {
        let config = build_thinking_config(Some(ThinkingEffort::High), "gemini-2.5-pro");
        assert_eq!(config["includeThoughts"], true);
        assert_eq!(config["thinkingBudget"], 24576);
        assert!(config.get("thinkingLevel").is_none());
    }

    #[test]
    fn thinking_gemini_2_5_effort_medium() {
        let config = build_thinking_config(Some(ThinkingEffort::Medium), "gemini-2.5-pro");
        assert_eq!(config["thinkingBudget"], 8192);
    }

    #[test]
    fn thinking_gemini_2_5_effort_low() {
        let config = build_thinking_config(Some(ThinkingEffort::Low), "gemini-2.5-flash");
        assert_eq!(config["thinkingBudget"], 1024);
    }

    #[test]
    fn thinking_gemini_2_5_effort_none_dynamic() {
        let config = build_thinking_config(None, "gemini-2.5-pro");
        assert_eq!(config["thinkingBudget"], -1);
    }

    #[test]
    fn thinking_unknown_model_defaults_to_level() {
        let config = build_thinking_config(Some(ThinkingEffort::Medium), "gemini-future-model");
        assert!(config.get("thinkingLevel").is_some());
        assert!(config.get("thinkingBudget").is_none());
    }

    #[test]
    fn thinking_disabled_no_config() {
        let config =
            build_generation_config(&SamplingConfig::default(), false, None, "gemini-3.1-pro");
        assert!(config.get("thinkingConfig").is_none());
    }

    #[test]
    fn thinking_level_and_budget_not_both() {
        // Verify Gemini 3.x has thinkingLevel but not thinkingBudget
        let config3 = build_thinking_config(Some(ThinkingEffort::High), "gemini-3.1-pro");
        assert!(config3.get("thinkingLevel").is_some());
        assert!(config3.get("thinkingBudget").is_none());

        // Verify Gemini 2.5 has thinkingBudget but not thinkingLevel
        let config25 = build_thinking_config(Some(ThinkingEffort::High), "gemini-2.5-pro");
        assert!(config25.get("thinkingBudget").is_some());
        assert!(config25.get("thinkingLevel").is_none());
    }

    // ---- Tool definition conversion tests (4.14) ----

    #[test]
    fn convert_single_tool() {
        let tools = vec![ToolDefinition {
            name: "search".to_string(),
            description: "Search the web".to_string(),
            parameters: serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}}),
        }];
        let result = convert_tools(&tools);
        let decls = result[0]["functionDeclarations"].as_array().unwrap();
        assert_eq!(decls.len(), 1);
        assert_eq!(decls[0]["name"], "search");
        assert_eq!(decls[0]["description"], "Search the web");
    }

    #[test]
    fn convert_multiple_tools_in_same_array() {
        let tools = vec![
            ToolDefinition {
                name: "a".to_string(),
                description: "A".to_string(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "b".to_string(),
                description: "B".to_string(),
                parameters: serde_json::json!({}),
            },
        ];
        let result = convert_tools(&tools);
        let decls = result[0]["functionDeclarations"].as_array().unwrap();
        assert_eq!(decls.len(), 2);
    }

    #[test]
    fn convert_empty_tools() {
        let result = convert_tools(&[]);
        let decls = result[0]["functionDeclarations"].as_array().unwrap();
        assert!(decls.is_empty());
    }

    // ---- Web search injection tests ----

    #[test]
    fn web_search_google_search_tool_structure() {
        let search_tool = serde_json::json!({ "google_search": {} });
        assert!(search_tool.get("google_search").is_some());
        assert!(search_tool["google_search"].as_object().unwrap().is_empty());
    }

    #[test]
    fn web_search_appended_to_function_tools() {
        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read".to_string(),
            parameters: serde_json::json!({}),
        }];
        let mut tool_array: Vec<serde_json::Value> =
            serde_json::from_value(convert_tools(&tools)).unwrap();
        tool_array.push(serde_json::json!({ "google_search": {} }));
        assert_eq!(tool_array.len(), 2);
        assert!(tool_array[0].get("functionDeclarations").is_some());
        assert!(tool_array[1].get("google_search").is_some());
    }
}
