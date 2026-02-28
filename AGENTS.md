# AGENTS.md

This file provides guidance to AI coding agents (Codex, Copilot, etc.) when working with code in this repository.

## Project Overview

krew-cli is a multi-AI-agent collaborative CLI tool written in Rust. Users chat with multiple LLMs (GPT, Claude, Gemini, etc.) simultaneously in one terminal using `@` addressing. See [PDD](docs/PDD.md) for product design and [TDD](docs/TDD.md) for technical design.

## Build & Development Commands

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build

# Run
cargo run                      # Run the CLI (krew-cli crate)

# Test
cargo test                     # Run all tests
cargo test -p krew-core        # Run tests for a specific crate
cargo test test_name           # Run a single test by name

# Lint & Format
cargo fmt --all                # Format all code
cargo fmt --all -- --check     # Check formatting (CI)
cargo clippy --all-targets --all-features -- -D warnings  # Lint with warnings as errors

# Check (fast compile check without codegen)
cargo check --all-targets
```

Always run `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings` before committing.

## Architecture

Cargo workspace with 6 crates under `crates/`:

```txt
krew-cli          CLI entry + TUI (clap, ratatui)
  └── krew-core   Session management, Agent Loop, @ routing, slash commands
        ├── krew-llm      LLM provider abstraction (OpenAI/Anthropic/Google/OpenAI-Compatible)
        ├── krew-tools    Tool trait + built-in tools (read/write/edit/shell/glob/grep) + MCP client
        ├── krew-storage  TOML session persistence (.krew/sessions/)
        └── krew-config   TOML config loading (.krew/settings.toml)
```

### Key Design Patterns

- **LlmClient trait** (`krew-llm`): All providers implement `async fn chat_stream()` returning `Stream<Item = StreamEvent>`. StreamEvent variants: TextDelta, ToolCall, ThinkingDelta, Done, Error.
- **Tool trait** (`krew-tools`): `fn parameters_schema() -> serde_json::Value` + `async fn execute(args) -> Result<ToolResult>`. Built-in tools enforce path boundary (must be within session cwd).
- **Agent Loop**: Serial execution per `reply_order` — each agent completes its full loop (including tool calls) before the next starts, so later agents see earlier agents' responses.
- **Message routing**: `parse_input()` returns `(Addressee, String)` where Addressee is `All | Single(name) | LastRespondent`.
- **Error types**: `anyhow` for application-level errors (`krew-cli`, `krew-core`), `thiserror` for library crate error definitions (`krew-llm`, `krew-tools`, `krew-storage`, `krew-config`).

### OpenAI Dual API Support

OpenAI agents have an `api_type` config field: `"responses"` (Responses API) or `"chat"` (Chat Completions API). Each has a separate implementation file (`openai_responses.rs`, `openai_chat.rs`). Azure mode activates when `azure_endpoint` is set.

## Conventions

- **Rust Edition 2024**, async runtime is **tokio**
- All workspace dependencies managed in root `Cargo.toml` with `[workspace.dependencies]` and `default-features = false`
- Use `reqwest` with `rustls` feature (no OpenSSL) for HTTP
- SSE streaming via `eventsource-stream` crate
- Config/session files use TOML format (via `toml` crate)
- Static linking: `static_vcruntime` on Windows, musl + `mimalloc` on Linux, `crt-static` on macOS
- All communication with user is in Chinese (中文)
