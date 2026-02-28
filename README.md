# krew-cli

A CLI tool for multi-AI-agent collaborative conversations. Run multiple AI models (GPT, Claude, Gemini, etc.) in a single terminal session — like hosting an AI roundtable meeting.

## Features

- **Multi-Agent Sessions** — Chat with multiple AI models simultaneously in one terminal
- **@ Addressing** — `@all` to broadcast, `@agent_name` to target a specific agent
- **Shared Context** — All agents see the full conversation history, enabling cross-agent collaboration
- **Built-in Tools** — File read/write, shell execution, glob/grep search
- **MCP Integration** — Extend agent capabilities via Model Context Protocol servers
- **Session Persistence** — Save and resume conversations with `/resume` and `/new`
- **Streaming Output** — Real-time token-by-token rendering with per-agent color coding

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

[[agents]]
name = "gpt"
display_name = "GPT-5.2"
provider = "openai"
model = "gpt-5.2"
api_type = "responses"
color = "green"
tools = true
enable_web_search = false

[[agents]]
name = "opus"
display_name = "Claude Opus"
provider = "anthropic"
model = "claude-opus-4-6"
color = "magenta"
tools = true
enable_web_search = false

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
| `/new` | Start a new session |
| `/resume` | Resume a previous session |
| `/agents` | List active agents |
| `/clear` | Clear screen |
| `/compact <agent>` | Compress context using the specified agent |
| `/help` | Show help |
| `/quit` | Exit |

### Tool Approval Levels

| Level | Read ops | Write ops | Shell / MCP |
| ----- | -------- | --------- | ----------- |
| `suggest` | Auto | Confirm | Confirm |
| `auto-edit` | Auto | Auto | Confirm |
| `full-auto` | Auto | Auto | Auto |

## Architecture

Built in Rust with a modular workspace structure:

```txt
krew-cli/
├── crates/
│   ├── krew-cli/        # CLI entry + TUI (ratatui)
│   ├── krew-core/       # Session, agent loop, routing
│   ├── krew-llm/        # LLM client abstraction
│   ├── krew-tools/      # Built-in tools + MCP
│   ├── krew-storage/    # TOML session persistence
│   └── krew-config/     # Config loading
└── docs/
    ├── PDD.md           # Product design
    └── TDD.md           # Technical design
```

See [PDD](docs/PDD.md) and [TDD](docs/TDD.md) for detailed design documents.

## License

Apache-2.0
