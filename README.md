<div align="center">

# krew-cli

**A CLI tool for multi-AI-agent collaborative conversations.**

Run multiple AI models (GPT, Claude, Gemini, etc.) in a single terminal session — like hosting an AI roundtable meeting.

[![CI](https://github.com/ZHing2006/krew-cli/actions/workflows/release.yml/badge.svg)](https://github.com/ZHing2006/krew-cli/actions/workflows/release.yml)
[![npm](https://img.shields.io/npm/v/@zhing2026/krew)](https://www.npmjs.com/package/@zhing2026/krew)
[![npm downloads](https://img.shields.io/npm/dm/@zhing2026/krew)](https://www.npmjs.com/package/@zhing2026/krew)
[![GitHub stars](https://img.shields.io/github/stars/ZHing2006/krew-cli)](https://github.com/ZHing2006/krew-cli/stargazers)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/built%20with-Rust-dea584)](https://www.rust-lang.org/)

[English](README.md) | [中文](README_CN.md)

</div>

<p align="center">
  <img src="docs/images/demo_en.gif" alt="krew-cli demo" width="800">
</p>

---

## Features

- **Multi-Agent Sessions** — Chat with multiple AI models simultaneously in one terminal
- **@ Addressing** — `@all` to broadcast, `@name` to target, no prefix to continue with last respondent
- **# Whisper** — `#name` for private messages invisible to other agents, multi-target whisper groups
- **Shared Context** — All agents share conversation history (whisper messages are filtered by visibility), enabling cross-agent collaboration
- **AI-to-AI Routing** — Agents can `@mention` each other, triggering automatic dispatch (immediate/queued strategies)
- **Built-in Tools** — File read/write/edit, shell execution, glob/grep search, URL fetch, skill activation
- **MCP Integration** — Extend agent capabilities via Model Context Protocol servers (stdio + HTTP)
- **Skill System** — Discoverable, activatable skills with `SKILL.md` definitions for specialized instructions
- **Custom Commands** — User-defined slash commands via Markdown files with argument substitution and bash preprocessing
- **Config Wizard** — Interactive `krew config init` setup, `config doctor` diagnostics, CRUD management for providers/agents
- **Session Persistence** — Save and resume conversations; `/rewind` to fork from any point in history
- **Token Tracking & Auto-Compact** — Real-time token usage; automatic context compression with whisper message preservation
- **Prompt Mode (`-p`)** — Non-interactive mode for scripts and CI/CD, with text/JSON output and stdin pipe support
- **Streaming Output** — ~60Hz token-by-token rendering with Markdown, syntax highlighting, and per-agent color coding
- **Thinking/Reasoning** — Display model thinking process (configurable effort: low/medium/high)
- **Web Search** — Provider-native web search (OpenAI Responses, OpenAI Chat, Anthropic, Gemini)
- **Per-Agent Sampling** — Configure temperature, top_p, max_tokens, etc. per agent
- **Project Instructions** — `AGENTS.md` files auto-injected into system prompts (hierarchical loading)

## Install

### npm (recommended)

```bash
npm install -g @zhing2026/krew
```

### GitHub Releases

Download the binary for your platform from [GitHub Releases](https://github.com/ZHing2006/krew-cli/releases).

| Platform | Binary |
| -------- | ------ |
| Windows x64 | `krew-win32-x64.exe` |
| Linux x64 | `krew-linux-x64` |
| Linux arm64 | `krew-linux-arm64` |
| macOS x64 | `krew-darwin-x64` |
| macOS arm64 | `krew-darwin-arm64` |

All binaries are statically linked — zero external dependencies.

### Build from source

```bash
cargo install --path crates/krew-cli
```

## Supported Providers

| Provider | Models (examples) | API |
| -------- | ----------------- | --- |
| OpenAI | GPT-5.2 | Responses / Chat Completions |
| Anthropic | Claude Opus 4.6, Sonnet 4.6 | Messages |
| Google | Gemini 3.1 Pro | generateContent (+ Vertex AI) |
| OpenAI-Compatible | Doubao, LiteLLM, etc. | Responses / Chat Completions |

## Quick Start

### Option A: Interactive setup (recommended)

**1. Install**

```bash
npm install -g @zhing2026/krew
```

**2. Run the config wizard**

```bash
krew config init
```

The wizard walks you through everything step by step:

```
=== User Configuration (Providers) ===

Add provider [1]
Select provider type:
> Anthropic
  OpenAI
  Google
  OpenAI-Compatible
Provider name [anthropic]: ↵
API key storage method:
> Environment variable
  Store in config file
Environment variable name [ANTHROPIC_API_KEY]: ↵
Base URL [https://api.anthropic.com]: ↵
Added provider "anthropic" (Anthropic)
Add another provider? (y/N): N

=== Project Configuration (Agents) ===

Select setup mode:
> Smart Preset
  Manual Setup
Fetching available models...
Select preset:
> Single Agent
  Three Agents
Select model: claude-sonnet-4-6 (anthropic)
Enable thinking for claude? (Y/n): Y
Write this configuration? (Y/n): Y
```

> **Tip:** You can choose "Store in config file" instead of "Environment variable" — the wizard will prompt for your API key directly and save it to the config file, no env var needed.

**3. Run**

```bash
krew
```

### Option B: Manual config file

```bash
npm install -g @zhing2026/krew
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

Create `~/.krew/settings.toml` (user-level — shared across all projects):

```toml
[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"

[providers.anthropic]
type = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
```

Create `.krew/settings.toml` in your project directory (project-level — agents):

```toml
[settings]
reply_order = ["gpt", "opus"]

[[agents]]
name = "gpt"
display_name = "GPT-5.2"
provider = "openai"
model = "gpt-5.2"
api_type = "responses"
color = "green"
tools = true

[[agents]]
name = "opus"
display_name = "Claude Opus"
provider = "anthropic"
model = "claude-opus-4-6"
color = "magenta"
tools = true
```

```bash
krew
```

## Usage

```
krew [OPTIONS] [COMMAND]

Options:
  -c, --config <PATH>           Config file path
  -a, --agents <NAMES>          Agents to enable (comma-separated)
      --approval-mode <MODE>    Tool approval: suggest | auto-edit | full-auto
      --resume [ID]             Resume a session
  -p, --prompt <PROMPT>         Non-interactive prompt mode
      --format <FORMAT>         Output format for -p mode: text | json
  -v, --verbose                 Verbose output
  -h, --help                    Help
  -V, --version                 Version

Commands:
  config init [--user|--project]   Interactive configuration setup
  config add <provider|agent>      Add a provider or agent
  config del <provider|agent>      Delete a provider or agent
  config list <providers|agents>   List providers or agents
  config doctor                    Diagnose configuration issues
  config help                      Print full configuration manual
```

### Addressing

```
› @all What's the best data structure for a message queue in Rust?
› @opus Can you elaborate on the lock-free ring buffer?
› Tell me more                    # sends to the last respondent
› #opus What do you think of GPT's approach?   # whisper (private)
```

### Slash Commands

| Command | Description |
| ------- | ----------- |
| `/clear` | Clear screen and start new session (alias: `/new`) |
| `/resume` | Resume a previous session |
| `/rewind` | Rewind to a previous message and fork the conversation |
| `/agents` | List active agents and token usage |
| `/compact [agent]` | Compress context using the specified agent |
| `/mcp` | List MCP servers and tools |
| `/skills` | List available skills |
| `/stats` | Show process statistics (memory, threads) |
| `/help` | Show all commands (including custom commands) |
| `/exit` | Exit (alias: `/quit`) |

### Prompt Mode

```bash
# Single agent
krew -p "@opus explain ownership in Rust"

# Pipe stdin
cat src/main.rs | krew -p "@opus review this code"

# JSON output
krew -p "@all hello" --format json

# Whisper in prompt mode
krew -p "#opus what do you think of GPT's approach?"
```

### Tool Approval

| Level | Read ops | Write ops | Shell | fetch_url | MCP |
| ----- | -------- | --------- | ----- | --------- | --- |
| `suggest` | Auto | Confirm | Confirm* | Allowlist auto | Confirm |
| `auto-edit` | Auto | Auto | Confirm* | Allowlist auto | Confirm |
| `full-auto` | Auto | Auto | Auto | Auto | Auto |

\* Shell commands matching `shell_allow_commands` prefixes are auto-approved.

## Architecture

Rust · Tokio · Ratatui — 6-crate workspace, 5-platform static binaries.

```
krew-cli          CLI entry + TUI (ratatui, crossterm)
  └── krew-core   Session, agent loop, routing, slash commands, skills, custom commands
        ├── krew-llm      LLM provider abstraction (OpenAI/Anthropic/Google/Compatible)
        ├── krew-tools    Tool trait + built-in tools (8) + MCP client (rmcp)
        ├── krew-storage  TOML session persistence + input history
        └── krew-config   TOML config loading + AGENTS.md instructions
```

**Documentation:**
- [User Manual](docs/MANUAL.md) ([中文](docs/MANUAL_CN.md)) — Installation, configuration, usage guide
- [PDD](docs/PDD.md) — Product design
- [TDD](docs/TDD.md) — Technical design

## License

[Apache-2.0](LICENSE)
