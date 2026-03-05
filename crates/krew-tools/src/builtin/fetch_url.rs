//! Fetch URL tool: fetches a web page and returns content as Markdown.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{ToolContext, ToolError, ToolHandler, ToolResult, ToolSpec};

/// Maximum response body size (1 MB).
const MAX_RESPONSE_SIZE: usize = 1_024 * 1_024;

/// Request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// User-Agent header value.
const USER_AGENT: &str = "krew-cli/0.1.0";

/// Built-in tool for fetching web pages and converting to Markdown.
#[derive(Default)]
pub struct FetchUrlTool;

#[derive(Deserialize)]
struct FetchUrlArgs {
    url: String,
}

impl FetchUrlTool {
    pub fn new() -> Self {
        Self
    }

    pub fn spec(&self) -> ToolSpec {
        ToolSpec {
            name: "fetch_url".to_string(),
            description: "Fetch a web page and return its content as Markdown. \
                          Use this to read documentation, articles, or any web content."
                .to_string(),
            parameters: json!({
                "type": "object",
                "required": ["url"],
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch (HTTP URLs are auto-upgraded to HTTPS)"
                    }
                },
                "additionalProperties": false
            }),
        }
    }
}

/// Normalize a URL: upgrade HTTP to HTTPS, validate format.
fn normalize_url(url: &str) -> Result<String, ToolError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(ToolError::InvalidArgs("URL cannot be empty".to_string()));
    }

    // Auto-upgrade http to https.
    let url = if url.starts_with("http://") {
        url.replacen("http://", "https://", 1)
    } else if url.starts_with("https://") {
        url.to_string()
    } else {
        // Assume https if no scheme.
        format!("https://{url}")
    };

    // Basic validation: must have a host.
    if extract_host(&url).is_none() {
        return Err(ToolError::InvalidArgs(format!(
            "invalid URL: cannot extract host from '{url}'"
        )));
    }

    Ok(url)
}

/// Extract the host from a URL string.
fn extract_host(url: &str) -> Option<String> {
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host_port = after_scheme.split('/').next()?;
    let host = host_port.split(':').next()?;
    if host.is_empty() {
        return None;
    }
    Some(host.to_lowercase())
}

/// Convert HTML to Markdown using htmd.
fn html_to_markdown(html: &str) -> String {
    htmd::convert(html).unwrap_or_else(|_| html.to_string())
}

#[async_trait::async_trait]
impl ToolHandler for FetchUrlTool {
    fn name(&self) -> &str {
        "fetch_url"
    }

    fn requires_approval(&self) -> bool {
        // Dynamic: depends on URL domain.
        // Default to true; the caller checks domain allowlist separately.
        true
    }

    async fn execute(&self, args: Value, _ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        let args: FetchUrlArgs = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidArgs(format!("invalid arguments: {e}")))?;

        let url = normalize_url(&args.url)?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .user_agent(USER_AGENT)
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|e| ToolError::Execution(format!("failed to create HTTP client: {e}")))?;

        let response = client.get(&url).send().await.map_err(|e| {
            if e.is_timeout() {
                ToolError::Execution(format!(
                    "request timed out after {REQUEST_TIMEOUT_SECS}s: {url}"
                ))
            } else {
                ToolError::Execution(format!("failed to fetch '{url}': {e}"))
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            return Ok(ToolResult {
                content: format!("HTTP {status} when fetching '{url}'"),
                is_error: true,
            });
        }

        // Read body with size limit.
        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::Execution(format!("failed to read response body: {e}")))?;

        let truncated = bytes.len() > MAX_RESPONSE_SIZE;
        let body_bytes = if truncated {
            &bytes[..MAX_RESPONSE_SIZE]
        } else {
            &bytes[..]
        };

        let html = String::from_utf8_lossy(body_bytes);
        let mut markdown = html_to_markdown(&html);

        if truncated {
            markdown.push_str("\n\n[Content truncated: response exceeded 1MB limit]");
        }

        let char_count = markdown.chars().count();
        markdown.push_str(&format!("\n\n({char_count} chars)"));

        Ok(ToolResult {
            content: markdown,
            is_error: false,
        })
    }
}

/// Check whether a fetch_url call should skip approval based on domain allowlist.
pub fn is_fetch_domain_allowed(args: &Value, allow_domains: &[String]) -> bool {
    let url = args.get("url").and_then(|u| u.as_str()).unwrap_or("");
    let normalized = match normalize_url(url) {
        Ok(u) => u,
        Err(_) => return false,
    };
    let host = match extract_host(&normalized) {
        Some(h) => h,
        None => return false,
    };
    allow_domains
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
}
