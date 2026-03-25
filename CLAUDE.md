# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

krew-cli is a multi-AI-agent collaborative CLI tool written in Rust. Users chat with multiple LLMs (GPT, Claude, Gemini, etc.) simultaneously in one terminal using `@` addressing and `#` whisper (private messages). See [PDD](docs/PDD.md) for product design and [TDD](docs/TDD.md) for technical design.

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

IMPORTANT: Always run `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings` before committing.

## Architecture

Cargo workspace with 6 crates under `crates/`:

```txt
krew-cli          CLI entry + TUI (clap, ratatui)
  └── krew-core   Session management, Agent Loop, routing, slash commands
        ├── krew-llm      LLM provider abstraction (OpenAI/Anthropic/Google/OpenAI-Compatible)
        ├── krew-tools    Tool system (8 built-in tools) + MCP client
        ├── krew-storage  TOML session persistence (.krew/sessions/)
        └── krew-config   TOML config loading (.krew/settings.toml)
```

### Key Design Patterns

- **Agent Loop**: Serial execution per `reply_order` — each agent completes its full loop (including tool calls) before the next starts, so later agents see earlier agents' responses.
- **Message routing**: `@name` addressing + `#name` whisper (private messages). Whisper messages are only visible to target agents; others see placeholders.
- **Built-in tools** enforce path boundary — all file operations must be within session cwd.
- **Error types**: `anyhow` for application-level errors (`krew-cli`, `krew-core`), `thiserror` for library crate error definitions (`krew-llm`, `krew-tools`, `krew-storage`, `krew-config`).

### OpenAI Dual API Support

OpenAI agents have an `api_type` config field: `"responses"` (Responses API) or `"chat"` (Chat Completions API). Each has a separate implementation file (`openai_responses.rs`, `openai_chat.rs`).

## Conventions

- **Rust Edition 2024**, async runtime is **tokio**
- All workspace dependencies managed in root `Cargo.toml` with `[workspace.dependencies]` and `default-features = false`
- Use `reqwest` with `rustls` feature (no OpenSSL) for HTTP
- SSE streaming via `eventsource-stream` crate
- Config/session files use TOML format (via `toml` crate)
- Static linking: `static_vcruntime` on Windows, musl + `mimalloc` on Linux, `crt-static` on macOS
- All communication with user is in Chinese (中文)
- All comments in code and config files must be in English
- Always use the `git` agent (Task tool with subagent_type "git") for all git operations (commit, push, etc.)

## Versioning

When bumping the version, **all** of the following files must be updated in sync:

1. **Cargo crates** (6 files, `version = "x.y.z"` on line 3):
   - `crates/krew-cli/Cargo.toml`
   - `crates/krew-config/Cargo.toml`
   - `crates/krew-core/Cargo.toml`
   - `crates/krew-llm/Cargo.toml`
   - `crates/krew-storage/Cargo.toml`
   - `crates/krew-tools/Cargo.toml`
2. **npm packages** (6 files, `"version"` field + dependency versions in main package):
   - `npm/krew/package.json` (version + 5 optionalDependencies versions)
   - `npm/krew-win32-x64/package.json`
   - `npm/krew-linux-x64/package.json`
   - `npm/krew-linux-arm64/package.json`
   - `npm/krew-darwin-x64/package.json`
   - `npm/krew-darwin-arm64/package.json`
3. **Git tag** — create a `v{VERSION}` tag on the release commit
