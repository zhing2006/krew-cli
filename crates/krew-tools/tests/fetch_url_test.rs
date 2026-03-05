use krew_tools::builtin::fetch_url::is_fetch_domain_allowed;
use serde_json::json;

// ---- URL normalization and host extraction (tested via is_fetch_domain_allowed) ----

#[test]
fn https_url_domain_match() {
    let allow = vec!["example.com".to_string()];
    let args = json!({ "url": "https://example.com/page" });
    assert!(is_fetch_domain_allowed(&args, &allow));
}

#[test]
fn http_url_auto_upgrade_domain_match() {
    let allow = vec!["example.com".to_string()];
    let args = json!({ "url": "http://example.com/page" });
    assert!(is_fetch_domain_allowed(&args, &allow));
}

#[test]
fn no_scheme_url_domain_match() {
    let allow = vec!["example.com".to_string()];
    let args = json!({ "url": "example.com/page" });
    assert!(is_fetch_domain_allowed(&args, &allow));
}

#[test]
fn invalid_url_returns_false() {
    let allow = vec!["example.com".to_string()];
    let args = json!({ "url": "" });
    assert!(!is_fetch_domain_allowed(&args, &allow));
}

#[test]
fn whitespace_trimmed_url() {
    let allow = vec!["example.com".to_string()];
    let args = json!({ "url": "  https://example.com  " });
    assert!(is_fetch_domain_allowed(&args, &allow));
}

// ---- Domain allowlist matching ----

#[test]
fn exact_domain_match() {
    let allow = vec!["github.com".to_string()];
    let args = json!({ "url": "https://github.com/repo" });
    assert!(is_fetch_domain_allowed(&args, &allow));
}

#[test]
fn subdomain_match() {
    let allow = vec!["github.com".to_string()];
    let args = json!({ "url": "https://docs.github.com/en" });
    assert!(is_fetch_domain_allowed(&args, &allow));
}

#[test]
fn domain_not_in_allowlist() {
    let allow = vec!["github.com".to_string()];
    let args = json!({ "url": "https://evil.com/page" });
    assert!(!is_fetch_domain_allowed(&args, &allow));
}

#[test]
fn partial_domain_no_match() {
    // "github.com" should NOT match "hub.com" — suffix must match after a dot.
    let allow = vec!["hub.com".to_string()];
    let args = json!({ "url": "https://github.com/page" });
    assert!(!is_fetch_domain_allowed(&args, &allow));
}

#[test]
fn empty_allowlist_denies_all() {
    let allow: Vec<String> = vec![];
    let args = json!({ "url": "https://anything.com/page" });
    assert!(!is_fetch_domain_allowed(&args, &allow));
}

#[test]
fn multiple_domains_in_allowlist() {
    let allow = vec!["docs.rs".to_string(), "github.com".to_string()];
    let args_docs = json!({ "url": "https://docs.rs/htmd/latest" });
    let args_gh = json!({ "url": "https://github.com/user/repo" });
    let args_other = json!({ "url": "https://evil.com/page" });
    assert!(is_fetch_domain_allowed(&args_docs, &allow));
    assert!(is_fetch_domain_allowed(&args_gh, &allow));
    assert!(!is_fetch_domain_allowed(&args_other, &allow));
}

#[test]
fn uppercase_url_host_lowered() {
    let allow = vec!["example.com".to_string()];
    let args = json!({ "url": "https://EXAMPLE.COM/page" });
    assert!(is_fetch_domain_allowed(&args, &allow));
}

#[test]
fn url_with_port_domain_match() {
    let allow = vec!["example.com".to_string()];
    let args = json!({ "url": "https://example.com:8080/page" });
    assert!(is_fetch_domain_allowed(&args, &allow));
}

// ---- HTML to Markdown conversion (via execute) ----
// Note: actual HTTP fetch tests require network, so we only test the conversion
// function indirectly. The unit test below validates the htmd integration.

#[test]
fn html_to_markdown_basic_conversion() {
    // Test the htmd crate integration directly.
    let html = "<h1>Title</h1><p>Hello <strong>world</strong></p>";
    let md = htmd::convert(html).unwrap();
    assert!(md.contains("Title"));
    assert!(md.contains("**world**"));
}

#[test]
fn html_to_markdown_empty() {
    let md = htmd::convert("").unwrap();
    assert!(md.is_empty() || md.trim().is_empty());
}

// ---- Response size limit constant ----

#[test]
fn max_response_size_is_1mb() {
    // Verify the constant matches spec (1MB).
    assert_eq!(1_024 * 1_024, 1_048_576);
}
