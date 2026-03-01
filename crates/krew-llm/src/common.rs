//! Common utilities shared across LLM provider implementations.
//!
//! Contains retry logic, HTTP status classification, error extraction,
//! and message merging for providers that require strict role alternation.

use crate::LlmError;

// ---------------------------------------------------------------------------
// Retry constants
// ---------------------------------------------------------------------------

/// Maximum retries for 429 rate limit responses (exponential backoff).
pub const MAX_RETRIES_429: u32 = 3;

/// Maximum retries for 5xx server error responses (fixed interval).
pub const MAX_RETRIES_5XX: u32 = 2;

/// Fixed retry interval for 5xx server errors.
pub const RETRY_INTERVAL_5XX: std::time::Duration = std::time::Duration::from_secs(2);

/// Timeout for first token / initial response.
pub const FIRST_TOKEN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

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

/// Send a request with retry logic for rate limits, server errors, and timeouts.
///
/// Implements:
/// - 429 exponential backoff: 1s → 2s → 4s, max 3 retries
/// - 5xx fixed interval: 2s between retries, max 2 retries
/// - Timeout: 60s, retry once on timeout
pub async fn send_with_retry(
    config: &RequestConfig<'_>,
    auth: &AuthMode<'_>,
    extra_headers: Option<&[(String, String)]>,
) -> Result<reqwest::Response, LlmError> {
    let mut retries_429: u32 = 0;
    let mut retries_5xx: u32 = 0;

    loop {
        let resp = tokio::time::timeout(FIRST_TOKEN_TIMEOUT, {
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
                    "{} request timed out after {FIRST_TOKEN_TIMEOUT:?}, retrying once",
                    config.provider_name,
                );
                let retry = tokio::time::timeout(FIRST_TOKEN_TIMEOUT, {
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

        match classify_status(status) {
            RetryAction::RateLimit if retries_429 < MAX_RETRIES_429 => {
                retries_429 += 1;
                let delay = std::time::Duration::from_secs(1 << (retries_429 - 1));
                tracing::warn!(
                    "{} 429 rate limit, retry {retries_429}/{MAX_RETRIES_429} after {delay:?}",
                    config.provider_name,
                );
                tokio::time::sleep(delay).await;
            }
            RetryAction::ServerError if retries_5xx < MAX_RETRIES_5XX => {
                retries_5xx += 1;
                tracing::warn!(
                    "{} {status} server error, retry {retries_5xx}/{MAX_RETRIES_5XX} after {RETRY_INTERVAL_5XX:?}",
                    config.provider_name,
                );
                tokio::time::sleep(RETRY_INTERVAL_5XX).await;
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
