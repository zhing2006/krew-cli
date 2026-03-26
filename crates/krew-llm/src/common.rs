//! Common utilities shared across LLM provider implementations.
//!
//! Contains retry logic, HTTP status classification, error extraction,
//! and message merging for providers that require strict role alternation.

use krew_config::RetryConfig;

use crate::LlmError;

// ---------------------------------------------------------------------------
// HTTP status classification
// ---------------------------------------------------------------------------

/// Classify an HTTP status code for retry decisions.
pub enum RetryAction {
    /// Retry with exponential backoff (429 rate limit).
    RateLimit,
    /// Retry with fixed interval (5xx server error).
    ServerError,
    /// Do not retry (auth error).
    AuthError,
    /// Do not retry (other client error).
    NoRetry,
}

/// Classify an HTTP status code into a retry action.
pub fn classify_status(status: reqwest::StatusCode) -> RetryAction {
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        RetryAction::RateLimit
    } else if status.is_server_error() {
        RetryAction::ServerError
    } else if status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
    {
        RetryAction::AuthError
    } else {
        RetryAction::NoRetry
    }
}

// ---------------------------------------------------------------------------
// Error message extraction
// ---------------------------------------------------------------------------

/// Extract error message from an HTTP error response body.
///
/// Attempts to parse the body as JSON with `{"error": {"message": "..."}}` structure.
/// Falls back to raw body text or status code only.
pub async fn extract_error_message(resp: reqwest::Response) -> String {
    let status = resp.status();
    match resp.text().await {
        Ok(body) => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body)
                && let Some(msg) = v
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|m| m.as_str())
            {
                return format!("{status}: {msg}");
            }
            format!("{status}: {body}")
        }
        Err(_) => format!("{status}"),
    }
}

// ---------------------------------------------------------------------------
// Retry-enabled request sending
// ---------------------------------------------------------------------------

/// Configuration for building an HTTP request for retry.
pub struct RequestConfig<'a> {
    /// HTTP client.
    pub http: &'a reqwest::Client,
    /// Target URL.
    pub url: &'a str,
    /// Request body.
    pub body: &'a serde_json::Value,
    /// Provider name for log messages.
    pub provider_name: &'a str,
}

/// Authentication mode for the request.
pub enum AuthMode<'a> {
    /// `Authorization: Bearer {token}` header.
    Bearer(&'a str),
    /// Custom header (e.g. `x-api-key`, `api-key`).
    Header(&'a str, &'a str),
}

// ---------------------------------------------------------------------------
// Retry notification
// ---------------------------------------------------------------------------

/// Information about a retry attempt, passed to the `on_retry` callback.
#[derive(Debug, Clone)]
pub struct RetryInfo {
    /// Current retry attempt (1-based).
    pub attempt: u32,
    /// Maximum attempts allowed for this error type.
    pub max_attempts: u32,
    /// Human-readable reason for the retry.
    pub reason: String,
    /// Delay in seconds before the retry.
    pub delay_secs: f64,
}

/// Send a request with configurable retry logic for rate limits, server errors,
/// and timeouts.
///
/// Retry behavior is controlled by `retry_config`. The optional `on_retry`
/// callback is invoked before each retry sleep to notify the caller (e.g. TUI).
pub async fn send_with_retry(
    config: &RequestConfig<'_>,
    auth: &AuthMode<'_>,
    extra_headers: Option<&[(String, String)]>,
    retry_config: &RetryConfig,
    on_retry: Option<&(dyn Fn(RetryInfo) + Send + Sync)>,
) -> Result<reqwest::Response, LlmError> {
    let mut retries_429: u32 = 0;
    let mut retries_5xx: u32 = 0;
    let timeout = std::time::Duration::from_secs(retry_config.request_timeout_secs);

    loop {
        let resp = tokio::time::timeout(timeout, {
            let mut req = config.http.post(config.url);
            req = apply_auth(req, auth);
            if let Some(headers) = extra_headers {
                for (k, v) in headers {
                    req = req.header(k.as_str(), v.as_str());
                }
            }
            req.json(config.body).send()
        })
        .await;

        let resp = match resp {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                return Err(LlmError::Network(e));
            }
            Err(_) => {
                tracing::warn!(
                    "{} request timed out after {timeout:?}, retrying once",
                    config.provider_name,
                );
                if let Some(cb) = &on_retry {
                    cb(RetryInfo {
                        attempt: 1,
                        max_attempts: 1,
                        reason: "timeout".into(),
                        delay_secs: 0.0,
                    });
                }
                let retry = tokio::time::timeout(timeout, {
                    let mut req = config.http.post(config.url);
                    req = apply_auth(req, auth);
                    if let Some(headers) = extra_headers {
                        for (k, v) in headers {
                            req = req.header(k.as_str(), v.as_str());
                        }
                    }
                    req.json(config.body).send()
                })
                .await;
                match retry {
                    Ok(Ok(r)) => r,
                    Ok(Err(e)) => return Err(LlmError::Network(e)),
                    Err(_) => {
                        return Err(LlmError::Api("request timed out after retry".into()));
                    }
                }
            }
        };

        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }

        let max_429 = retry_config.max_retries_rate_limit;
        let max_5xx = retry_config.max_retries_server_error;

        match classify_status(status) {
            RetryAction::RateLimit if retries_429 < max_429 => {
                retries_429 += 1;
                let delay_secs = retry_config.backoff_base_secs
                    * retry_config
                        .backoff_multiplier
                        .powi((retries_429 - 1) as i32);
                let delay = std::time::Duration::from_secs_f64(delay_secs);
                tracing::warn!(
                    "{} 429 rate limit, retry {retries_429}/{max_429} after {delay:.1?}",
                    config.provider_name,
                );
                if let Some(cb) = &on_retry {
                    cb(RetryInfo {
                        attempt: retries_429,
                        max_attempts: max_429,
                        reason: "rate limit (429)".into(),
                        delay_secs,
                    });
                }
                tokio::time::sleep(delay).await;
            }
            RetryAction::ServerError if retries_5xx < max_5xx => {
                retries_5xx += 1;
                let delay_secs = retry_config.server_error_interval_secs;
                let delay = std::time::Duration::from_secs_f64(delay_secs);
                tracing::warn!(
                    "{} {status} server error, retry {retries_5xx}/{max_5xx} after {delay:.1?}",
                    config.provider_name,
                );
                if let Some(cb) = &on_retry {
                    cb(RetryInfo {
                        attempt: retries_5xx,
                        max_attempts: max_5xx,
                        reason: format!("server error ({status})"),
                        delay_secs,
                    });
                }
                tokio::time::sleep(delay).await;
            }
            RetryAction::AuthError => {
                let msg = extract_error_message(resp).await;
                return Err(LlmError::Auth(msg));
            }
            _ => {
                let msg = extract_error_message(resp).await;
                return Err(LlmError::Api(msg));
            }
        }
    }
}

/// Apply authentication to a request builder.
fn apply_auth(req: reqwest::RequestBuilder, auth: &AuthMode<'_>) -> reqwest::RequestBuilder {
    match auth {
        AuthMode::Bearer(token) => req.bearer_auth(token),
        AuthMode::Header(name, value) => req.header(*name, *value),
    }
}

// ---------------------------------------------------------------------------
// Consecutive same-role message merging
// ---------------------------------------------------------------------------

/// A simple role + content pair for merging.
#[derive(Debug, Clone)]
pub struct RoleContent {
    pub role: String,
    pub content: String,
}

/// Merge consecutive messages with the same role.
///
/// When multiple messages in a row have the same role, their content is joined
/// with `\n\n`. This is required by providers that enforce strict role
/// alternation (Anthropic, Gemini).
pub fn merge_consecutive_same_role(messages: Vec<RoleContent>) -> Vec<RoleContent> {
    if messages.is_empty() {
        return Vec::new();
    }

    let mut merged: Vec<RoleContent> = Vec::with_capacity(messages.len());

    for msg in messages {
        if let Some(last) = merged.last_mut()
            && last.role == msg.role
        {
            last.content.push_str("\n\n");
            last.content.push_str(&msg.content);
        } else {
            merged.push(msg);
        }
    }

    merged
}

// ---------------------------------------------------------------------------
// Base64 encoding helper
// ---------------------------------------------------------------------------

/// Encode raw bytes as a base64 string (standard alphabet, with padding).
pub fn encode_base64(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_429_as_rate_limit() {
        assert!(matches!(
            classify_status(reqwest::StatusCode::TOO_MANY_REQUESTS),
            RetryAction::RateLimit
        ));
    }

    #[test]
    fn classify_500_as_server_error() {
        assert!(matches!(
            classify_status(reqwest::StatusCode::INTERNAL_SERVER_ERROR),
            RetryAction::ServerError
        ));
    }

    #[test]
    fn classify_502_as_server_error() {
        assert!(matches!(
            classify_status(reqwest::StatusCode::BAD_GATEWAY),
            RetryAction::ServerError
        ));
    }

    #[test]
    fn classify_503_as_server_error() {
        assert!(matches!(
            classify_status(reqwest::StatusCode::SERVICE_UNAVAILABLE),
            RetryAction::ServerError
        ));
    }

    #[test]
    fn classify_401_as_auth_error() {
        assert!(matches!(
            classify_status(reqwest::StatusCode::UNAUTHORIZED),
            RetryAction::AuthError
        ));
    }

    #[test]
    fn classify_403_as_auth_error() {
        assert!(matches!(
            classify_status(reqwest::StatusCode::FORBIDDEN),
            RetryAction::AuthError
        ));
    }

    #[test]
    fn classify_400_as_no_retry() {
        assert!(matches!(
            classify_status(reqwest::StatusCode::BAD_REQUEST),
            RetryAction::NoRetry
        ));
    }

    #[test]
    fn classify_404_as_no_retry() {
        assert!(matches!(
            classify_status(reqwest::StatusCode::NOT_FOUND),
            RetryAction::NoRetry
        ));
    }

    #[test]
    fn classify_200_success() {
        // 200 is not an error status — classify still returns NoRetry
        // (caller checks is_success() before calling classify).
        assert!(matches!(
            classify_status(reqwest::StatusCode::OK),
            RetryAction::NoRetry
        ));
    }

    #[test]
    fn merge_two_consecutive_user_messages() {
        let msgs = vec![
            RoleContent {
                role: "user".into(),
                content: "[agentA] foo".into(),
            },
            RoleContent {
                role: "user".into(),
                content: "[agentB] bar".into(),
            },
        ];
        let result = merge_consecutive_same_role(msgs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "user");
        assert_eq!(result[0].content, "[agentA] foo\n\n[agentB] bar");
    }

    #[test]
    fn merge_three_consecutive_user_messages() {
        let msgs = vec![
            RoleContent {
                role: "user".into(),
                content: "a".into(),
            },
            RoleContent {
                role: "user".into(),
                content: "b".into(),
            },
            RoleContent {
                role: "user".into(),
                content: "c".into(),
            },
        ];
        let result = merge_consecutive_same_role(msgs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "a\n\nb\n\nc");
    }

    #[test]
    fn no_merge_alternating_roles() {
        let msgs = vec![
            RoleContent {
                role: "user".into(),
                content: "hi".into(),
            },
            RoleContent {
                role: "assistant".into(),
                content: "hello".into(),
            },
        ];
        let result = merge_consecutive_same_role(msgs);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn merge_mixed_pattern() {
        // user/user/assistant/assistant/user → 3 messages
        let msgs = vec![
            RoleContent {
                role: "user".into(),
                content: "u1".into(),
            },
            RoleContent {
                role: "user".into(),
                content: "u2".into(),
            },
            RoleContent {
                role: "assistant".into(),
                content: "a1".into(),
            },
            RoleContent {
                role: "assistant".into(),
                content: "a2".into(),
            },
            RoleContent {
                role: "user".into(),
                content: "u3".into(),
            },
        ];
        let result = merge_consecutive_same_role(msgs);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].content, "u1\n\nu2");
        assert_eq!(result[1].content, "a1\n\na2");
        assert_eq!(result[2].content, "u3");
    }

    #[test]
    fn merge_single_message() {
        let msgs = vec![RoleContent {
            role: "user".into(),
            content: "hello".into(),
        }];
        let result = merge_consecutive_same_role(msgs);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hello");
    }

    #[test]
    fn merge_empty_list() {
        let result = merge_consecutive_same_role(Vec::new());
        assert!(result.is_empty());
    }
}
