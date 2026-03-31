## 1. Config Layer

- [x] 1.1 Add `extra_headers: Option<HashMap<String, String>>` field to `ProviderConfig` in `crates/krew-config/src/lib.rs`
- [x] 1.2 Add `extra_headers: Vec<(String, String)>` field to `LlmClientConfig` in `crates/krew-llm/src/lib.rs`

## 2. Provider Clients

- [x] 2.1 Add `extra_headers` field to `GoogleClient` struct, accept in `new()`, pass to `send_with_retry()` in `chat_stream()` (`crates/krew-llm/src/google.rs`)
- [x] 2.2 Add `extra_headers` field to `AnthropicClient` struct, accept in `new()`, merge with hardcoded headers in `chat_stream()` (`crates/krew-llm/src/anthropic.rs`)
- [x] 2.3 Add `extra_headers` field to `OpenAiChatClient` struct, accept in `new()`, pass to `send_with_retry()` in `chat_stream()` (`crates/krew-llm/src/openai_chat.rs`)
- [x] 2.4 Add `extra_headers` field to `OpenAiResponsesClient` struct, accept in `new()`, pass to `send_with_retry()` in `chat_stream()` (`crates/krew-llm/src/openai_responses.rs`)

## 3. Core Wiring

- [x] 3.1 Update provider creation logic in `krew-core` to pass `extra_headers` from `ProviderConfig` through `LlmClientConfig` to each client constructor

## 4. CLI Help & Config Writer

- [x] 4.1 Update `krew config help` provider section in `crates/krew-cli/src/config_cmd/help.rs` — add `extra_headers` field description
- [x] 4.2 Update config writer in `crates/krew-config/src/writer.rs` — support serializing `extra_headers` when writing provider config
- [x] 4.3 Update `crates/krew-cli/tests/config_help_test.rs` — add `extra_headers` to expected help output if needed

## 5. Documentation

- [x] 5.1 Update `config.example.toml` — add `extra_headers` example to Google Vertex AI provider section
- [x] 5.2 Update `docs/MANUAL.md` — add `extra_headers` to §5.4 Provider configuration, note scope (chat/inference only) and conflicting headers warning
- [x] 5.3 Update `docs/MANUAL_CN.md` — add `extra_headers` to §5.4 Provider 配置, note scope and conflicting headers warning
- [x] 5.4 Update `README.md` and `README_CN.md` — skipped, README only shows minimal config examples
- [x] 5.5 Update `docs/PDD.md` — add `extra_headers` to §4.9.2 config example
- [x] 5.6 Update `docs/TDD.md` — add `extra_headers` to §3.3.2 Provider implementation details

## 6. Verification

- [x] 6.1 Run `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings`
- [x] 6.2 Run `cargo test` to ensure no regressions
