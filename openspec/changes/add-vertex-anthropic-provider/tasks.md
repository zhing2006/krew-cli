## 1. Config Types and Writer

- [x] 1.1 Add `ProviderType::VertexAnthropic` with per-variant `#[serde(rename = "vertex-anthropic")]` so existing `rename_all = "lowercase"` behavior for `OpenAI`, `Anthropic` and `Google` stays unchanged
- [x] 1.2 Update provider type labels and display helpers to include `Vertex Anthropic`
- [x] 1.3 Update `ProviderWriteData` / `add_provider()` serialization to write `type = "vertex-anthropic"`
- [x] 1.4 Add config deserialization and writer tests for `vertex-anthropic`

## 2. LLM Client

- [x] 2.1 Refactor reusable Anthropic protocol helpers to `pub(crate)` functions without changing existing Anthropic behavior or crate public API
- [x] 2.2 Run existing Anthropic unit tests after helper extraction, using current `web_search_20250305` tests as regression baseline
- [x] 2.3 Add `VertexAnthropicClient` with Bearer auth, `anthropic_version = "vertex-2023-10-16"` body field and no top-level `model`
- [x] 2.4 Implement URL builder for Google Vertex hosts (`global` -> `aiplatform.googleapis.com`, `us` -> `aiplatform.us.rep.googleapis.com`, `eu` -> `aiplatform.eu.rep.googleapis.com`, regional -> `{location}-aiplatform.googleapis.com`) and `base_url` passthrough roots
- [x] 2.5 Add Vertex Anthropic `enable_web_search` injection using `{ "type": "web_search_20250305", "name": "web_search" }`
- [x] 2.6 Wire `ProviderType::VertexAnthropic` into agent initialization and skip agents with missing `vertex_project` or `vertex_location`
- [x] 2.7 Add unit tests for URL construction, request body fields, Bearer auth, web search tool type, reused SSE parsing and agent factory creation/skip behavior

## 3. Model Listing

- [x] 3.1 Extend `ListModelsConfig` handling for `ProviderType::VertexAnthropic`
- [x] 3.2 Implement Vertex Anthropic publisher model listing for Google official endpoint with correct `global`, `us`/`eu` and regional host selection
- [x] 3.3 Implement list-models URL construction for LiteLLM passthrough and generic custom `base_url`
- [x] 3.4 Add fallback models for `ProviderType::VertexAnthropic`
- [x] 3.5 Add unit tests for model ID extraction, filtering, sorting, fallback, official host selection and passthrough URL construction

## 4. Config Wizard and CLI

- [x] 4.1 Add `Vertex Anthropic` to `krew config init` provider type selection
- [x] 4.2 Collect Bearer token source, `vertex_project`, `vertex_location` and optional `base_url` for Vertex Anthropic providers
- [x] 4.3 Add `Vertex Anthropic` support to `krew config add provider`
- [x] 4.4 Update `krew config list providers` type label and key status display
- [x] 4.5 Update Smart Preset / manual agent creation to fetch and select Vertex Anthropic models
- [x] 4.6 Add CLI wizard tests for provider collection and generated config

## 5. Documentation and Help

- [x] 5.1 Update `krew config help` text with `vertex-anthropic` field semantics and examples
- [x] 5.2 Update README provider matrix and setup examples
- [x] 5.3 Update `docs/MANUAL.md` and `docs/MANUAL_CN.md` with Google official Vertex and LiteLLM passthrough examples
- [x] 5.4 Update `docs/TDD.md` LLM Provider boundary section with the new provider, and add/update URL/auth tables for Vertex Anthropic host selection, Bearer auth and web search tool type
- [x] 5.5 If this change includes a release/version bump, update all 6 Cargo crate versions, all 6 npm package versions/dependency pins and create the matching `v{VERSION}` git tag
  - Version files updated to `0.11.11`; create `v0.11.11` after the release commit exists.

## 6. Verification

- [x] 6.1 Run targeted Rust tests for `krew-config`, `krew-llm` and `krew-cli` config flows
- [x] 6.2 Run `cargo fmt --all`
- [x] 6.3 Run `cargo clippy --all-targets --all-features -- -D warnings`
- [x] 6.4 Run `cargo test`
