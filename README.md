# krew-cli

A CLI tool for multi-AI-agent collaborative conversations. Run multiple AI models (GPT, Claude, Gemini, etc.) in a single terminal session — like hosting an AI roundtable meeting.

## Features

- **Multi-Agent Sessions** — Chat with multiple AI models simultaneously in one terminal
- **@ Addressing** — `@all` to broadcast, `@agent_name` to target a specific agent
- **Shared Context** — All agents see the full conversation history, enabling cross-agent collaboration
- **Built-in Tools** — File read/write, shell execution, glob/grep search, URL fetch (HTML→Markdown)
- **MCP Integration** — Extend agent capabilities via Model Context Protocol servers (stdio + HTTP)
- **Session Persistence** — Save and resume conversations with `/resume` and `/clear`
- **Token Tracking & Auto-Compact** — Real-time token usage display; automatic context compression when threshold is exceeded
- **Streaming Output** — Real-time token-by-token rendering with per-agent color coding and status bar
- **Thinking/Reasoning** — Display model thinking process (configurable effort: low/medium/high)
- **Web Search** — Provider-native web search (OpenAI Responses, Anthropic, Gemini)
- **Per-Agent Sampling** — Configure temperature, top_p, max_tokens, etc. per agent

## Install

### npm (recommended)

```bash
npm install -g @zhing2026/krew
```

### GitHub Releases

Download the binary for your platform from [GitHub Releases](https://github.com/zhing2006/krew-cli/releases).

| Platform | Binary |
| -------- | ------ |
| Windows x64 | `krew-win32-x64.exe` |
| Linux x64 | `krew-linux-x64` |
| Linux arm64 | `krew-linux-arm64` |
| macOS x64 | `krew-darwin-x64` |
| macOS arm64 | `krew-darwin-arm64` |

### Build from source

```bash
cargo install --path crates/krew-cli
```

## Supported Providers

| Provider | Models (examples) | API |
| -------- | ----------------- | --- |
| OpenAI | GPT-5.2 | Responses / Chat Completions |
| Azure OpenAI | GPT-5.2 (Azure) | Responses / Chat Completions |
| Anthropic | Claude Opus 4.6, Sonnet 4.6 | Messages |
| Google | Gemini 3.1 Pro | generateContent |
| OpenAI-Compatible | Doubao, etc. | Responses / Chat Completions |

## Quick Start

### 1. Create a config file

Create `.krew/settings.toml` in your project directory:

```toml
[settings]
approval_mode = "suggest"
reply_order = ["gpt", "opus"]
auto_compact_threshold = 120000    # auto-compress context at 120K tokens (0 = disable)

[[agents]]
name = "gpt"
display_name = "GPT-5.2"
provider = "openai"
model = "gpt-5.2"
api_type = "responses"
color = "green"
tools = true
enable_web_search = false
enable_thinking = false
# sampling.temperature = 0.7
# sampling.max_tokens = 32768

[[agents]]
name = "opus"
display_name = "Claude Opus"
provider = "anthropic"
model = "claude-opus-4-6"
color = "magenta"
tools = true
enable_web_search = false
enable_thinking = false

[providers.openai]
api_key_env = "OPENAI_API_KEY"
base_url = "https://api.openai.com/v1"

[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
base_url = "https://api.anthropic.com"
```

### 2. Set your API keys

```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

### 3. Run

```bash
krew
```

## Usage

```txt
krew [OPTIONS]

Options:
  -c, --config <PATH>           Config file path
  -a, --agents <NAMES>          Agents to enable (comma-separated)
      --approval-mode <MODE>    Tool approval: suggest | auto-edit | full-auto
      --resume [ID]             Resume a session
  -v, --verbose                 Verbose output
  -h, --help                    Help
  -V, --version                 Version
```

### Addressing

```txt
you> @all What's the best data structure for a message queue in Rust?
you> @opus Can you elaborate on the lock-free ring buffer?
you> Tell me more          # sends to the last respondent
```

### Slash Commands

| Command | Description |
| ------- | ----------- |
| `/clear` | Clear screen and start new session (alias: `/new`) |
| `/resume` | Resume a previous session |
| `/agents` | List active agents and token usage |
| `/compact <agent>` | Compress context using the specified agent |
| `/mcp` | List MCP servers and tools |
| `/stats` | Show process statistics |
| `/help` | Show help |
| `/exit` | Exit (alias: `/quit`) |

### Tool Approval Levels

| Level | Read ops | Write ops | Shell | fetch_url | MCP |
| ----- | -------- | --------- | ----- | --------- | --- |
| `suggest` | Auto | Confirm | Confirm* | Allowlist auto | Confirm |
| `auto-edit` | Auto | Auto | Confirm* | Allowlist auto | Confirm |
| `full-auto` | Auto | Auto | Auto | Auto | Auto |

\* Shell commands matching `shell_allow_commands` prefixes are auto-approved.

## Architecture

Built in Rust with a modular workspace structure:

```txt
krew-cli/
├── crates/
│   ├── krew-cli/        # CLI entry + TUI (ratatui)
│   ├── krew-core/       # Session, agent loop, routing, compact
│   ├── krew-llm/        # LLM client abstraction (4 providers)
│   ├── krew-tools/      # Built-in tools (7) + MCP client
│   ├── krew-storage/    # TOML session persistence + input history
│   └── krew-config/     # Config loading + AGENTS.md instructions
├── .github/workflows/   # CI/CD (5-platform release builds)
├── npm/                 # npm distribution packages
└── docs/
    ├── PDD.md           # Product design
    └── TDD.md           # Technical design
```

See [PDD](docs/PDD.md) and [TDD](docs/TDD.md) for detailed design documents.

## License

Apache-2.0
