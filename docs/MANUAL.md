# krew-cli User Manual

> Version: 0.7.0

---

## Table of Contents

1. [Installation](#1-installation)
2. [Getting Started](#2-getting-started)
3. [Common Recipes](#3-common-recipes)
4. [Command-Line Arguments](#4-command-line-arguments)
5. [Configuration](#5-configuration)
6. [Config Management (`krew config`)](#6-config-management-krew-config)
7. [Addressing & Routing](#7-addressing--routing)
8. [Slash Commands](#8-slash-commands)
9. [Custom Commands](#9-custom-commands)
10. [Tool System](#10-tool-system)
11. [MCP Integration](#11-mcp-integration)
12. [Skill System](#12-skill-system)
13. [Sub-Agent System (Experimental)](#13-sub-agent-system-experimental)
14. [Session Management](#14-session-management)
15. [Prompt Mode](#15-prompt-mode)
16. [Project Instructions (AGENTS.md)](#16-project-instructions-agentsmd)
17. [File Paths & Load Priority](#17-file-paths--load-priority)
18. [Keyboard Shortcuts](#18-keyboard-shortcuts)
19. [Troubleshooting](#19-troubleshooting)

---

## 1. Installation

### npm (recommended)

```bash
npm install -g @zhing2026/krew
```

### GitHub Releases

Download the binary for your platform from [GitHub Releases](https://github.com/ZHing2006/krew-cli/releases):

| Platform | Binary |
| -------- | ------ |
| Windows x64 | `krew-win32-x64.exe` |
| Linux x64 | `krew-linux-x64` |
| Linux arm64 | `krew-linux-arm64` |
| macOS x64 | `krew-darwin-x64` |
| macOS arm64 | `krew-darwin-arm64` |

All binaries are statically linked with no external dependencies.

### Build from source

Requires Rust (edition 2024) and Cargo:

```bash
git clone https://github.com/ZHing2006/krew-cli.git
cd krew-cli
cargo install --path crates/krew-cli
```

### Verify installation

```bash
krew --version
```

---

## 2. Getting Started

This section walks you through your very first session with krew.

### Option A: Interactive setup (recommended)

The fastest way to get started — the config wizard handles everything:

```bash
krew config init
```

The wizard will:
1. Ask you to set up providers (choose type, enter API key)
2. Ask you to define agents (pick a provider, select a model)
3. Write `~/.krew/settings.toml` (user-level) and `.krew/settings.toml` (project-level)

Then just run `krew` to start chatting!

### Option B: Manual setup

#### Step 1: Create a config directory

**macOS / Linux:**
```bash
mkdir -p .krew
```

**Windows (PowerShell):**
```powershell
mkdir .krew -Force
```

#### Step 2: Create `.krew/settings.toml`

Create a file called `.krew/settings.toml` in your project directory. Here is the simplest possible config — one agent, one provider:

```toml
[settings]
reply_order = ["opus"]

[[agents]]
name = "opus"
display_name = "Claude Opus"
provider = "anthropic"
model = "claude-opus-4-6"
color = "magenta"
tools = true

[providers.anthropic]
type = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
```

#### Step 3: Set your API key

**macOS / Linux:**
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

**Windows (PowerShell):**
```powershell
$env:ANTHROPIC_API_KEY = "sk-ant-..."
```

**Windows (CMD):**
```cmd
set ANTHROPIC_API_KEY=sk-ant-...
```

> **Tip:** Add the export to your shell profile (`~/.bashrc`, `~/.zshrc`, or Windows System Environment Variables) so you don't have to set it every time.

#### Step 4: Run

```bash
krew
```

You should see a startup banner like this:

```
┌──────────────────────────────────────────────────┐
│ Krew CLI v0.6.0                                  │
│ Agents: [opus] Claude Opus                       │
│ Directory: /path/to/project  Type /help for ...  │
└──────────────────────────────────────────────────┘
›
```

Type a message (e.g. `@opus hello!`) and press Enter. You're chatting!

### What's next?

- Add more agents → `krew config add agent` or see [Recipe: Multi-provider setup](#recipe-1-multi-provider-setup-openai--anthropic)
- Try whisper mode → type `#opus secret message`
- Use tools → ask the agent to read a file or run a command
- Diagnose config issues → `krew config doctor`
- Learn all commands → type `/help` in the session

---

## 3. Common Recipes

### Recipe 1: Multi-provider setup (OpenAI + Anthropic)

Set up two agents from different providers so you can compare answers.

**1. Set API keys:**

```bash
# macOS / Linux
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```
```powershell
# Windows PowerShell
$env:OPENAI_API_KEY = "sk-..."
$env:ANTHROPIC_API_KEY = "sk-ant-..."
```

**2. Create `.krew/settings.toml`:**

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

[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"

[providers.anthropic]
type = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
```

**3. Try it:**

```
› @all What's the best way to handle errors in Rust?
```

Both agents respond in order. Later agents can see earlier agents' answers.

### Recipe 2: Share provider config across projects

Put providers and API keys in user-level config so every project can use them. Or use `krew config init` to set this up interactively.

**1. Create `~/.krew/settings.toml`:**

```toml
[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"

[providers.anthropic]
type = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"

[providers.google]
type = "google"
api_key_env = "GOOGLE_API_KEY"
```

**2. In each project, `.krew/settings.toml` only defines agents:**

```toml
[settings]
reply_order = ["gpt", "opus"]

[[agents]]
name = "gpt"
display_name = "GPT"
provider = "openai"
model = "gpt-5.2"
api_type = "responses"
color = "green"
tools = true

[[agents]]
name = "opus"
display_name = "Opus"
provider = "anthropic"
model = "claude-opus-4-6"
color = "magenta"
tools = true
```

krew merges both files automatically. No need to repeat provider config.

### Recipe 3: Resume and rewind a conversation

**Resume a previous session:**

```bash
# From the command line
krew --resume

# Or inside a running session
› /resume
```

A picker shows your recent sessions. Select one to continue where you left off.

**Rewind to an earlier point:**

```
› /rewind
```

A picker shows all your messages. Select one — everything after it is discarded, and when you send a new message, krew forks into a new session. The original session is untouched.

### Recipe 4: Use prompt mode in CI/CD

Run a one-shot prompt from a script and parse the output.

**Code review in CI:**

```bash
git diff HEAD~1 | krew -p "@opus review these changes for bugs" --format json
```

**Generate a changelog:**

```bash
git log --oneline v0.5.3..HEAD | krew -p "@gpt summarize these commits as a changelog"
```

**Check exit code:**

```bash
krew -p "@opus does this code have any security issues?" --format text
if [ $? -ne 0 ]; then
  echo "Agent errored" >&2
fi
```

Exit codes: `0` = success, `1` = agent error, `2` = argument/config error.

### Recipe 5: Private evaluation with whisper

Ask one agent to privately evaluate another's answer:

```
› @all Propose an architecture for a chat app
  (both agents answer publicly)

› #opus What are the weaknesses in GPT's proposal?
  (only opus sees this; other agents see a placeholder)
```

Multi-target whisper group — two agents discuss privately:

```
› #opus #gemini Discuss the tradeoffs between these approaches
  (only opus and gemini see each other's replies)
```

### Recipe 6: Auto-approve safe shell commands

Tired of confirming `ls` and `cargo build`? Add them to the allowlist:

```toml
[settings]
shell_allow_commands = ["ls", "cat", "cargo build", "cargo test", "git status", "git diff"]
```

These prefixes are matched — `cargo build --release` is also auto-approved.

---

## 4. Command-Line Arguments

```
krew [OPTIONS] [COMMAND]

Options:
  -c, --config <PATH>           Path to settings.toml (default: .krew/settings.toml)
  -a, --agents <NAMES>          Enable only these agents (comma-separated, e.g. "gpt,opus")
      --approval-mode <MODE>    Override tool approval mode: suggest | auto-edit | full-auto
      --resume [ID]             Resume a previous session (interactive picker if no ID given)
  -p, --prompt <PROMPT>         Non-interactive prompt mode (see §14)
      --format <FORMAT>         Output format for -p mode: text (default) | json
  -v, --verbose                 Enable debug-level logging
  -h, --help                    Show help
  -V, --version                 Show version

Commands:
  config init [--user|--project]   Interactive configuration setup
  config add <provider|agent>      Add a provider or agent
  config del <provider|agent>      Delete a provider or agent
  config list <providers|agents>   List providers or agents
  config doctor                    Diagnose configuration issues
  config help                      Print full configuration manual
```

### Examples

```bash
# Start with default config
krew

# Use only two agents
krew -a gpt,opus

# Resume last session
krew --resume

# Override approval mode
krew --approval-mode full-auto

# Non-interactive prompt
krew -p "@opus explain Rust ownership"

# Interactive configuration setup
krew config init

# Add a new provider
krew config add provider

# Diagnose configuration issues
krew config doctor
```

---

## 5. Configuration

### 5.1 Config file locations

krew uses a two-layer config system:

| File | Purpose |
| ---- | ------- |
| `~/.krew/settings.toml` | **User-level**: providers, API keys, preferences, global MCP servers |
| `.krew/settings.toml` | **Project-level**: agent definitions, reply_order, project overrides |

When both exist, they are merged. Project-level values take precedence:
- Same-name provider: project replaces user entirely
- Same-name MCP server: project replaces user
- Scalar settings: project wins
- `agents` and `reply_order` are only defined at the project level

The `--config` flag overrides the project config path but still merges with user-level config.

### 5.2 Settings

```toml
[settings]
# Tool approval mode (default: "suggest")
#   suggest   — read ops auto, write/shell/MCP need confirmation
#   auto-edit — read+write auto, shell/MCP need confirmation
#   full-auto — everything auto-approved
approval_mode = "suggest"

# Agent response order for @all broadcasts
reply_order = ["gpt", "opus", "gemini"]

# Auto-compact threshold in tokens (default: 120000, 0 = disable)
auto_compact_threshold = 120000

# Rounds to preserve during compact (default: 10)
compact_keep_rounds = 10

# Other agent messages role: "user" (default) or "assistant"
# other_agent_role = "user"

# Tokio worker threads (default: 4)
# worker_threads = 4

# Shell commands auto-approved by prefix match
# shell_allow_commands = ["ls", "cargo build", "git status"]

# Domains auto-approved for fetch_url (subdomain matching)
# fetch_allow_domains = ["docs.rs", "github.com"]

# AI-to-AI routing strategy: "immediate" (default) or "queued"
# agent_to_agent_routing = "immediate"

# Max AI-to-AI rounds (default: 10, 0 = disable)
# agent_to_agent_max_rounds = 10

# Restrict built-in file tools to the workspace directory (default: true)
# When false, file tools can access any path on the system.
# restrict_workspace = true

# Language for agent responses (unset = no instruction injected)
# Injects: "Always respond in {language}. Use {language} for all explanations,
# comments, and communications with the user. Technical terms and code
# identifiers should remain in their original form."
# language = "中文"
```

### 5.3 Agent definition

```toml
[[agents]]
name = "opus"                    # Unique name for @ addressing (required)
display_name = "Claude Opus"     # Display name in TUI (required)
provider = "anthropic"           # References a [providers.*] entry (required)
model = "claude-opus-4-6"        # Model ID (required)
color = "magenta"                # Terminal color: red/green/yellow/blue/magenta/cyan/white
system_prompt = ""               # Custom system prompt (optional)
tools = true                     # Enable tool use (default: false)
enable_web_search = false        # Enable provider-native web search (default: false)
enable_thinking = false          # Show model thinking/reasoning (default: false)
# thinking_effort = "medium"     # Thinking effort: low | medium | high

# OpenAI-specific
# api_type = "responses"         # "responses" (Responses API) or "chat" (Chat Completions)

# Sampling parameters (all optional, use provider defaults if unset)
# sampling.temperature = 0.7     # OpenAI/Google: 0-2, Anthropic: 0-1
# sampling.top_p = 0.9
# sampling.top_k = 40            # Anthropic/Google only
# sampling.max_tokens = 32768    # Default: model's max output
# sampling.frequency_penalty = 0
# sampling.presence_penalty = 0
# sampling.stop_sequences = ["END"]
```

**Reserved name:** `"all"` cannot be used as an agent name.

### 5.4 Provider configuration

Each provider requires a `type` field specifying the provider type: `"openai"`, `"anthropic"`, or `"google"`.

```toml
# OpenAI
[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"       # Environment variable name (recommended)
# api_key = "sk-..."                 # Direct key (not recommended)
# base_url = "https://api.openai.com"

# Anthropic
[providers.anthropic]
type = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
# base_url = "https://api.anthropic.com"

# Google (API key mode)
[providers.google]
type = "google"
api_key_env = "GOOGLE_API_KEY"

# Google (Vertex AI mode)
[providers.vertex]
type = "google"
vertex_project = "my-project"
vertex_location = "us-central1"

# OpenAI-Compatible (e.g. Doubao, LiteLLM)
[providers.doubao]
type = "openai"
api_key_env = "DOUBAO_API_KEY"
base_url = "https://ark.cn-beijing.volces.com/api/v3"
```

### 5.5 MCP server configuration

```toml
# Stdio transport (child process)
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
env = { SOME_VAR = "$ENV_VAR" }      # Supports $VAR expansion
trust = "auto"                        # "auto" = skip approval, "confirm" (default) = follow approval_mode

# HTTP transport (Streamable HTTP)
[[mcp_servers]]
name = "remote-tools"
url = "https://mcp.example.com/sse"
headers = { Authorization = "Bearer $TOKEN" }
```

### 5.6 Skill configuration

```toml
[skills]
enabled = true                        # Enable skill system (default: true)
# extra_paths = ["/path/to/skills"]   # Additional skill search paths
```

### 5.7 Config validation

krew validates the configuration on startup. Common errors:
- `reply_order` references an agent name not in `agents`
- Agent references a provider not in `providers`
- Duplicate agent names
- `"all"` used as agent name (reserved)

---

## 6. Config Management (`krew config`)

krew provides a suite of `config` subcommands for interactive configuration management. All config subcommands run in normal terminal mode (no TUI).

### 6.1 `krew config init`

Interactive wizard to set up configuration files from scratch.

```bash
krew config init              # Auto-detect what needs setup
krew config init --user       # Only set up user-level config (~/.krew/settings.toml)
krew config init --project    # Only set up project-level config (.krew/settings.toml)
```

**Smart routing** (no flags): the wizard checks which config files exist and only creates what's missing:
- Both missing → set up user config first, then project config
- User missing, project exists → set up user config only
- User exists, project missing → set up project config only
- Both exist → exit with message

**User config setup** guides you through adding providers:
1. Select provider type (Anthropic / OpenAI / Google / OpenAI-Compatible)
2. Enter provider name (auto-suggested based on type)
3. Choose API key method (environment variable or config file)
4. Optionally set base_url (OpenAI-Compatible) or Vertex fields (Google)
5. For OpenAI-Compatible providers: select API type — "Chat Completions (recommended)" (default) or "Responses API". Official OpenAI providers default to Responses API automatically without prompting.
6. Loop — add more providers or finish

> **Note:** If your `base_url` ends with `/v1` (e.g. `https://openrouter.ai/api/v1`), krew automatically strips the trailing `/v1` to avoid duplicate path segments in API requests.

**Project config setup** offers two modes:
- **Smart Preset**: fetches available models from your providers, offers 1-agent or 3-agent presets, lets you pick models via fuzzy search
- **Manual Setup**: loop-based agent creation — you pick provider, model, name, display name, color, thinking, and web search; `tools` is always enabled

### 6.2 `krew config add`

Add a single provider or agent to existing config.

```bash
krew config add provider    # Add a provider to user config
krew config add agent       # Add an agent to project config
```

Uses the same interactive prompts as `init`. The new entry is appended to the appropriate config file with format-preserving writes (comments and formatting are retained).

### 6.3 `krew config del`

Delete a provider or agent.

```bash
krew config del provider    # Delete a provider from user config
krew config del agent       # Delete an agent from project config
```

Shows a selection list, warns about dependencies (e.g. agents referencing a provider), and asks for confirmation before deleting.

### 6.4 `krew config list`

Display providers or agents in table format.

```bash
krew config list providers   # Show all providers with key status
krew config list agents      # Show all agents with their settings
```

**Providers table** shows: Name, Type, Key Method (with ✅/❌ for env var status), Base URL.

**Agents table** shows: Name, Display Name, Provider, Model, Color, Thinking, Web Search. Also displays `reply_order` below the table.

### 6.5 `krew config doctor`

Diagnose configuration completeness and validity.

```bash
krew config doctor
```

Checks:
- Config file existence and syntax (user + project)
- Provider API key availability (env var set? config file key present?)
- Agent provider references (does the referenced provider exist?)
- MCP server command availability (is the command in PATH?)

Uses ✅/❌/⚠️ indicators. Reports explicit parse errors (does not silently fall back like runtime loading).

### 6.6 `krew config help`

Print the full configuration manual to stdout.

```bash
krew config help
```

Covers: file locations, merge rules, all fields with defaults, CLI command reference, and example configurations.

---

## 7. Addressing & Routing

### @ addressing

| Syntax | Behavior |
| ------ | -------- |
| `@all <message>` | Broadcast to all agents (respond in `reply_order`) |
| `@name <message>` | Send to a specific agent |
| `@a @b <message>` | Send to multiple agents (in @ order) |
| `<message>` (no @) | Send to last respondent |

`@name` tokens can appear anywhere in the message. Unrecognized `@tokens` are treated as plain text. The full original message (including @ tokens) is sent to the LLM.

### # whisper (private messages)

| Syntax | Behavior |
| ------ | -------- |
| `#name <message>` | Whisper to one agent (others see placeholder) |
| `#a #b <message>` | Whisper group (only group members see content) |

- `#all` is rejected (returns error)
- `@` and `#` cannot be mixed in one message
- Agent replies to whispers automatically inherit whisper targets
- Whisper messages display a lock icon in TUI

### AI-to-AI routing

When an agent's reply `@mentions` another agent, that agent is automatically dispatched:

- **immediate** (default): target agent is inserted at the front of the queue
- **queued**: target agent is appended to the end of the queue
- Controlled by `agent_to_agent_routing` and `agent_to_agent_max_rounds`
- In whisper mode, only group members are routed; out-of-group mentions are ignored

---

## 8. Slash Commands

| Command | Description |
| ------- | ----------- |
| `/clear` | Clear screen and start new session (alias: `/new`) |
| `/resume` | List and resume a previous session |
| `/rewind` | Rewind to a previous message point (fork semantics) |
| `/agents` | List active agents with per-agent token usage |
| `/compact [agent]` | Compress context using specified agent (default: first in reply_order) |
| `/mcp` | List connected MCP servers and their tools |
| `/skills` | List available skills |
| `/tools` | List available tools per agent |
| `/stats` | Show process statistics (memory, threads) |
| `/help` | Show all commands (built-in + custom) |
| `/exit` | Exit program (alias: `/quit`) |

Built-in commands take priority over custom commands with the same name. Type `/` to see the completion popup.

---

## 9. Custom Commands

Define custom slash commands as Markdown files.

### Discovery paths (priority high to low)

| Priority | Path |
| -------- | ---- |
| 1 | `.krew/commands/` |
| 2 | `.agents/commands/` |
| 3 | `.claude/commands/` |
| 4 | `~/.krew/commands/` |
| 5 | `~/.agents/commands/` |
| 6 | `~/.claude/commands/` |

Subdirectories form namespaces: `commands/review/code.md` becomes `/review:code`. Same-name commands use first-found wins.

### File format

```markdown
---
description: Review code for issues
argument-hint: <file_path>
---

Please review the following file for potential issues: $ARGUMENTS
```

### Argument substitution

- `$ARGUMENTS` — full argument string
- `$1`, `$2`, ... — positional arguments (space-separated); missing positions become empty string

### Bash preprocessing

Use `` !`command` `` syntax to embed shell output:

```markdown
Analyze these changes:
!`git diff --cached`
```

The shell command runs in the session working directory. Failures are replaced with error messages (not aborted).

---

## 10. Tool System

### Built-in tools

| Tool | Description | Approval |
| ---- | ----------- | -------- |
| `read_file` | Read file content (with line numbers, offset/limit); also reads image files (png/jpg/jpeg/gif/webp) for AI vision | Auto |
| `glob` | File pattern matching | Auto |
| `grep` | Content search (regex, include filter) | Auto |
| `write_file` | Create/overwrite file (auto-creates parent dirs) | Configurable |
| `edit_file` | Search-and-replace edit (unique match validation, diff preview) | Configurable |
| `shell` | Execute shell command (timeout 120s, output limit 100KB) | Configurable |
| `fetch_url` | Fetch URL (HTTP→HTTPS upgrade, HTML→Markdown, 1MB limit) | Configurable |
| `activate_skill` | Activate a skill (auto-registered when skills exist) | Auto |

All file tools enforce path boundaries: operations must be within the session working directory.

### Shell tool details

- **Windows**: Uses Git Bash (searches: `KREW_BASH_PATH` → PATH bash.exe (skipping WSL) → `C:\Program Files\Git\bin\bash.exe`)
- **Unix**: Uses `KREW_BASH_PATH` → `$SHELL` → `/bin/sh`
- **Timeout**: `timeout_seconds` parameter (default 120s)
- **Output**: Truncated at 100KB with `[output truncated at 100KB]` marker

### fetch_url details

- HTTP URLs auto-upgrade to HTTPS
- Response size limit: 1MB
- Domain allowlist: `fetch_allow_domains` in settings; subdomain matching (e.g. `docs.github.com` matches `github.com`)
- Timeout: 30 seconds

### Approval behavior

| Mode | Read tools | Write tools | Shell | fetch_url | MCP tools |
| ---- | ---------- | ----------- | ----- | --------- | --------- |
| `suggest` | Auto | Confirm | Confirm* | Allowlist auto, else confirm | Confirm** |
| `auto-edit` | Auto | Auto | Confirm* | Allowlist auto, else confirm | Confirm** |
| `full-auto` | Auto | Auto | Auto | Auto | Auto |

\* Shell commands matching `shell_allow_commands` prefixes are auto-approved.
\** MCP tools with `trust = "auto"` skip approval; `trust = "confirm"` follows annotations.

**Approval shortcuts**: `y` approve / `a` approve for session / `n` or `Esc` deny / `Ctrl+C` abort

**Session cache**: "Approve for session" caches by command prefix (shell), host (fetch_url), or tool name (others).

---

## 11. MCP Integration

krew supports extending agent capabilities through [Model Context Protocol](https://modelcontextprotocol.io) servers.

### Configuration

See [§5.5](#55-mcp-server-configuration) for config syntax. Or run `krew config help` for a complete reference.

### Tool naming

MCP tools appear with qualified names: `mcp__<server>__<tool>` (internal) / `mcp:<server>/<tool>` (display).

### Trust levels

| Trust | Behavior |
| ----- | -------- |
| `auto` | All tools from this server skip approval |
| `confirm` (default) | Follow `approval_mode` rules, considering tool annotations |

MCP servers initialize at session startup. Use `/mcp` to list connected servers and their tools.

---

## 12. Skill System

Skills provide specialized instructions for specific tasks.

### Skill directory structure

```
my-skill/
├── SKILL.md            # Required: skill definition
├── scripts/            # Optional: helper scripts
├── references/         # Optional: reference materials
└── assets/             # Optional: other resources
```

### SKILL.md format

```markdown
---
name: code-review
description: Perform thorough code review with best practices
---

## Instructions
When reviewing code, check for...
```

Required fields: `name`, `description`. Optional: `compatibility`, `metadata`.

### Discovery paths

| Priority | Path |
| -------- | ---- |
| 1 | `.krew/skills/` |
| 2 | `.agents/skills/` |
| 3 | `.claude/skills/` |
| 4 | `~/.krew/skills/` |
| 5 | `~/.agents/skills/` |
| 6 | `~/.claude/skills/` |
| 7 | `skills.extra_paths` entries |

Scan depth: 4 levels. Skips: `.git/`, `node_modules/`, `target/`. Same-name skills: first-found wins.

### How skills work

1. On startup, krew discovers all skills and builds a catalog
2. The catalog is injected into each agent's system prompt
3. When an agent encounters a matching task, it calls `activate_skill` to load full instructions
4. The skill's instructions, directory path, and resource file listing are returned

Use `/skills` to list available skills.

---

## 13. Sub-Agent System (Experimental)

Sub-Agents allow agents to delegate focused tasks to child agents running in isolated contexts, keeping the main conversation free of intermediate tool call noise.

### Enable

In `settings.toml`:

```toml
[settings]
sub_agent_enabled = true
```

Disabled by default. When off, no agent definition files are read — zero overhead.

### Define a Sub-Agent

Place `.md` files in any of these directories:

- `.krew/agents/` — Project-level (highest priority)
- `.agents/agents/` — Project-level, cross-client
- `.claude/agents/` — Project-level, Claude Code compatible

User-level directories (`~/.krew/agents/` etc.) are also supported.

**Definition file format:**

```markdown
---
name: git
description: Git operations agent
color: cyan        # optional
maxTurns: 50       # optional, default 30
---

You are a git expert. Handle all git operations including
staging, committing, and pushing changes.
```

- `name` and `description` are required fields
- The YAML body (after `---`) becomes the Sub-Agent's system prompt
- Claude Code fields like `tools`, `model`, etc. are parsed but ignored

### How It Works

1. An agent calls the `run_agent` tool to invoke a Sub-Agent
2. The Sub-Agent runs in a fully isolated context (independent message history)
3. It shares the parent agent's tools (including MCP) and approval settings
4. Tool call events are streamed to the user in real-time
5. The final result is returned to the parent agent

### View Sub-Agents

Use the `/agents` command to see discovered Sub-Agent definitions.

---

## 14. Session Management

### Persistence

Every message is saved in real-time to `.krew/sessions/<session_id>.toml`. Even if krew crashes, your conversation is preserved.

### Resume a session

```bash
# Interactive picker
krew --resume

# Or from within krew
/resume
```

### Rewind (fork)

`/rewind` lets you go back to any previous user message and start a new branch:

1. A picker shows all user messages
2. Select a point to rewind to
3. Messages after that point are discarded
4. A new session ID is generated when you send your next message
5. The original session remains untouched

### Auto-compact

When `prompt_tokens` exceeds `auto_compact_threshold`, krew automatically compresses history before the next message:

- Uses `reply_order[0]` as the compression agent
- Preserves the last `compact_keep_rounds` rounds (default 10)
- Whisper messages are extracted and preserved (not compressed)
- A backup is created before compression

### Token tracking

Use `/agents` to see per-agent token usage (input/output/total).

---

## 15. Prompt Mode

Non-interactive mode for scripting and CI/CD.

### Basic usage

```bash
krew -p "@opus explain Rust ownership"
```

### Requirements

- Must include at least one `@agent`, `@all`, or `#agent` addressing
- `#all` is rejected
- No @ or # → exit code 2

### Stdin pipe

```bash
cat src/main.rs | krew -p "@opus review this"
git diff | krew -p "@gpt summarize changes"
```

Stdin content is wrapped in `<stdin>...</stdin>` tags and prepended to the prompt. Addressing is only parsed from the `-p` argument, not from stdin.

### Output formats

**Text** (default, `--format text`): Streaming output.

```
[opus]
⚡ read_file(src/main.rs)
   ⎿  done

This code has the following issues...
```

**JSON** (`--format json`): JSONL, one event per line.

```json
{"agent":"opus","type":"tool_start","tool":"read_file","arguments":"{\"file_path\":\"src/main.rs\"}"}
{"agent":"opus","type":"tool_output","text":"..."}
{"agent":"opus","type":"tool_done","tool":"read_file","summary":"done"}
{"agent":"opus","type":"text","content":"This code has the following issues..."}
```

Whisper mode adds `"whisper_targets": [...]` to JSON objects. Text format shows `[agent] [whisper]` header.

### Complete JSON event reference

| `type` | Fields | Description |
| ------ | ------ | ----------- |
| `text` | `agent`, `content` | Agent's text response (emitted once after streaming completes) |
| `tool_start` | `agent`, `tool`, `arguments` | A tool call is starting |
| `tool_output` | `agent`, `text` | Incremental tool output (e.g. shell streaming) |
| `tool_done` | `agent`, `tool`, `summary` | A tool call completed |
| `server_tool_start` | `agent`, `tool` | Server-side tool started (e.g. web search) |
| `server_tool_done` | `agent`, `tool`, `query` | Server-side tool completed |

All events include `"agent"` (agent name). Whisper events additionally include `"whisper_targets": [...]`.

**Example: full JSON session**

```json
{"agent":"opus","type":"server_tool_start","tool":"web_search"}
{"agent":"opus","type":"server_tool_done","tool":"web_search","query":"rust error handling best practices"}
{"agent":"opus","type":"tool_start","tool":"read_file","arguments":"{\"file_path\":\"src/main.rs\"}"}
{"agent":"opus","type":"tool_done","tool":"read_file","summary":"1,234 bytes"}
{"agent":"opus","type":"tool_start","tool":"shell","arguments":"{\"command\":\"cargo test\"}"}
{"agent":"opus","type":"tool_output","text":"running 42 tests\n"}
{"agent":"opus","type":"tool_output","text":"test result: ok. 42 passed\n"}
{"agent":"opus","type":"tool_done","tool":"shell","summary":"exit 0"}
{"agent":"opus","type":"text","content":"All tests pass. Here's my analysis..."}
```

### Exit codes

| Code | Meaning |
| ---- | ------- |
| 0 | All agents completed successfully |
| 1 | One or more agents errored (API error, etc.) |
| 2 | Argument/config error (missing addressing, conflicts, etc.) |

### Constraints

- `-p` and `--resume` are mutually exclusive
- All tools run in `full-auto` mode (no approval prompts)
- AI-to-AI routing is supported, subject to `agent_to_agent_max_rounds`

---

## 16. Project Instructions (AGENTS.md)

Place an `AGENTS.md` file in your project directory to provide all agents with project context (architecture, coding conventions, etc.).

### Discovery

krew walks from the working directory up to the filesystem root, collecting all `AGENTS.md` files found. They are merged ancestor-first (root → cwd), with child directories supplementing parent content.

### Injection

Content is wrapped in `<project-instructions>` tags and injected into every agent's system prompt, before the skill catalog and agent's own `system_prompt`.

### Limits

- Max file size: 100KB (truncated with warning if exceeded)
- Non-UTF-8 files are skipped with a warning log

---

## 17. File Paths & Load Priority

### Configuration files

```
Priority (high to low):
  CLI arguments (--approval-mode, --agents, etc.)
    ↓ overrides
  .krew/settings.toml          (project-level config)
    ↓ overrides
  ~/.krew/settings.toml         (user-level config)
    ↓ overrides
  Built-in defaults
```

### Data directories

| Path | Content |
| ---- | ------- |
| `.krew/settings.toml` | Project config |
| `.krew/sessions/` | Session TOML files |
| `.krew/history` | Input history (persists across sessions) |
| `.krew/logs/` | Log files (daily rolling, 7-day retention) |
| `~/.krew/settings.toml` | User config |

### Commands discovery (priority high to low)

| # | Path | Scope |
| - | ---- | ----- |
| 1 | `.krew/commands/` | Project, krew-specific |
| 2 | `.agents/commands/` | Project, cross-client |
| 3 | `.claude/commands/` | Project, Claude Code compatible |
| 4 | `~/.krew/commands/` | User, krew-specific |
| 5 | `~/.agents/commands/` | User, cross-client |
| 6 | `~/.claude/commands/` | User, Claude Code compatible |

### Skills discovery (priority high to low)

| # | Path | Scope |
| - | ---- | ----- |
| 1 | `.krew/skills/` | Project, krew-specific |
| 2 | `.agents/skills/` | Project, cross-client |
| 3 | `.claude/skills/` | Project, Claude Code compatible |
| 4 | `~/.krew/skills/` | User, krew-specific |
| 5 | `~/.agents/skills/` | User, cross-client |
| 6 | `~/.claude/skills/` | User, Claude Code compatible |
| 7 | `skills.extra_paths` entries | Config-specified |

### Project instructions discovery

```
AGENTS.md files loaded from:
  / (filesystem root)         ← merged first
    ↓
  /path/to/                   ← ancestor directories
    ↓
  /path/to/project/           ← working directory (merged last, highest priority)
```

All discovery uses **first-found wins** for same-name entries.

---

## 18. Keyboard Shortcuts

### Chat mode

| Key | Action |
| --- | ------ |
| `Enter` | Send message |
| `Shift+Enter` / `Ctrl+J` | New line |
| `↑` / `↓` | Browse input history |
| `@` | Open agent completion popup (includes "all") |
| `#` | Open whisper target popup (excludes "all") |
| `/` | Open slash command popup |
| `Esc` | Cancel current agent's streaming output |
| `Ctrl+C` (double) | Exit program |

### Completion popup

| Key | Action |
| --- | ------ |
| `↑` / `↓` | Navigate items |
| `Tab` / `Enter` | Confirm selection |
| `Esc` | Close popup |

### Approval overlay

| Key | Action |
| --- | ------ |
| `y` | Approve this time |
| `a` | Approve for session (same tool+context) |
| `n` / `Esc` | Deny |
| `Enter` | Confirm selected option |
| `↑` / `↓` | Navigate options |
| `Ctrl+C` | Abort entire agent turn |

---

## 19. Troubleshooting

### "Git Bash not found" on Windows

krew requires Git Bash for shell commands on Windows. Install [Git for Windows](https://git-scm.com/download/win) or set `KREW_BASH_PATH`:

```powershell
$env:KREW_BASH_PATH = "C:\Program Files\Git\bin\bash.exe"
```

### API key errors (401/403)

Ensure your API key environment variables are set:

```bash
# macOS / Linux
echo $OPENAI_API_KEY

# Windows PowerShell
echo $env:OPENAI_API_KEY
```

If the variable is set but you still get auth errors, check that the key hasn't expired and has the right permissions.

### Why does `#all` give an error?

`#all` (whisper to everyone) is deliberately rejected — whispering to all agents is semantically identical to a normal message, so it's disallowed to prevent confusion. Use `@all` instead for broadcasting.

### Why doesn't my message go to any agent?

If you type a message without `@` or `#` and there's no previous respondent (e.g. at the start of a session), krew doesn't know who to send it to. Use `@name` or `@all` to specify a target.

### Why does shell keep asking for approval?

Shell commands are confirmed by default in `suggest` and `auto-edit` modes. To auto-approve common commands, add them to the allowlist:

```toml
[settings]
shell_allow_commands = ["ls", "cargo", "git status", "git diff"]
```

The matching is **prefix-based**: `"cargo"` auto-approves `cargo build`, `cargo test`, etc. To skip all approval, use `--approval-mode full-auto` (use with caution).

After pressing `a` (approve for session), the same command prefix won't ask again in the current session.

### Config validation errors

Run `krew config doctor` for a comprehensive diagnostic, or use `--verbose` to see detailed error messages:

```bash
krew config doctor
krew --verbose
```

Common causes:
- `reply_order` mentions an agent name that isn't defined in `[[agents]]`
- An agent's `provider` doesn't match any `[providers.*]` entry
- Two agents have the same `name`
- An agent is named `"all"` (reserved word)

### Token limit / context length errors

If the LLM returns a context length error, your conversation is too long. Options:

1. **Manual compress**: `/compact` (or `/compact opus` to pick an agent)
2. **Lower the threshold**: Set `auto_compact_threshold = 80000` for earlier auto-compression
3. **Start fresh**: `/clear` to begin a new session

### Where are the log files?

Logs are written to `.krew/logs/` in the project directory. Files rotate daily and are automatically cleaned up after 7 days. Use `--verbose` for debug-level detail.

### My custom command isn't showing up

Check that:
1. The `.md` file is in one of the [discovery paths](#16-file-paths--load-priority) (e.g. `.krew/commands/my-command.md`)
2. The file has valid YAML frontmatter (or no frontmatter at all — it's optional)
3. There isn't a built-in command with the same name (built-in commands take priority)
4. Restart krew after adding new command files

### Agent doesn't use tools

Make sure `tools = true` is set in the agent config. Also check that the agent's provider supports tool use (all built-in providers do).
