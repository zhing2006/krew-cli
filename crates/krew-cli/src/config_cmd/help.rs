pub async fn run() -> anyhow::Result<()> {
    print!("{MANUAL}");
    Ok(())
}

const MANUAL: &str = r#"
=== krew Configuration Manual ===

krew-cli is a multi-AI-agent collaborative CLI tool. Users chat with multiple
LLMs (GPT, Claude, Gemini, etc.) simultaneously in one terminal.

Configuration is stored in TOML files at two levels that are merged at startup.

────────────────────────────────────────────────────────────────────────────────
1. FILE LOCATIONS & MERGE RULES
────────────────────────────────────────────────────────────────────────────────

  User config:    ~/.krew/settings.toml
  Project config: .krew/settings.toml   (relative to working directory)

Supported sections per level:

  Section            User config    Project config
  ─────────────────  ────────────   ──────────────
  [settings]         ✓ (no reply_order)  ✓ (with reply_order)
  [providers.*]      ✓              ✓
  [[agents]]         ✗              ✓
  [[mcp_servers]]    ✓              ✓
  [skills]           ✓              ✓

Merge rules (project config takes precedence):
  • providers    — merged by key; project replaces user's same-name provider
  • mcp_servers  — merged by name; same-name entries use project's definition
  • settings     — each scalar: project value wins if set, otherwise inherits user
  • skills       — project wins if set, otherwise inherits user
  • agents       — project config only (user config has no agents)
  • reply_order  — project config only

────────────────────────────────────────────────────────────────────────────────
2. CONFIGURATION REFERENCE
────────────────────────────────────────────────────────────────────────────────

─── [settings] ────────────────────────────────────────────────────────────────

  approval_mode              "suggest" | "auto-edit" | "full-auto"
                             Default: "suggest"
                             Tool approval policy.
                               suggest   — read ops auto, write/shell/MCP need confirmation
                               auto-edit — read+write auto, shell/MCP need confirmation
                               full-auto — all operations execute without confirmation

  reply_order                Array of agent names, e.g. ["claude", "gpt"]
                             (project config only)
                             Determines the order agents respond to @all messages.

  auto_compact_threshold     Integer (tokens) or omit to disable
                             When set, auto-compacts conversation when token count exceeds
                             this threshold.

  compact_keep_rounds        Integer
                             Default: 3
                             Number of recent conversation rounds to keep during compaction.

  input_history_limit        Integer
                             Default: 1000
                             Maximum number of input history entries to persist.

  paste_burst_detection      Boolean
                             Default: true
                             Enable timing-based paste detection as fallback when the
                             terminal does not support bracketed paste.

  worker_threads             Integer
                             Default: 4
                             Number of tokio worker threads for the async runtime.

  other_agent_role           "user" | "assistant"
                             Default: "user"
                             How other agents' messages appear in each agent's history.

  agent_to_agent_routing     "immediate" | "queued"
                             Default: "immediate"
                             AI-to-AI routing strategy when an agent @-mentions another.
                               immediate — insert target to queue head for next response
                               queued    — append target to queue tail

  agent_to_agent_max_rounds  Integer
                             Default: 10
                             Maximum AI-to-AI routing rounds per user message turn.

  language                   String or omit
                             Default: (none)
                             Language for agent responses (e.g. "Chinese", "English").
                             When set, a language instruction is injected into every
                             agent's system prompt.

  restrict_workspace         Boolean
                             Default: true
                             Restrict built-in file tools (read_file, write_file,
                             edit_file, glob, grep) to the workspace directory.
                             When false, file tools can access any path on the system.

  sub_agent_enabled          Boolean
                             Default: false
                             Enable the Sub-Agent feature (experimental).
                             When true, discovers agent definitions from
                             .krew/agents/, .agents/agents/, .claude/agents/
                             and registers the run_agent tool for delegating
                             tasks to isolated sub-agents.

  update_check               Boolean
                             Default: true
                             Check npm registry for new versions on startup.
                             When a newer version is available, displays a
                             warning with the upgrade command. Results are
                             cached for 24 hours. Set to false to disable.

─── [settings.retry] ─────────────────────────────────────────────────────────

  max_retries_rate_limit     Integer
                             Default: 3
                             Maximum retries for 429 rate-limit responses.

  max_retries_server_error   Integer
                             Default: 2
                             Maximum retries for 5xx server error responses.

  backoff_base_secs          Float
                             Default: 2.0
                             Base delay in seconds for exponential backoff (429).

  backoff_multiplier         Float
                             Default: 3.0
                             Multiplier for exponential backoff (429).

  server_error_interval_secs Float
                             Default: 2.0
                             Fixed retry interval in seconds for 5xx errors.

  request_timeout_secs       Integer
                             Default: 60
                             Request timeout in seconds for initial response / first token.

─── [[allow_rules]] / [[deny_rules]] / [[ask_rules]] (top-level) ──────────────

  Permission rules for fine-grained tool approval control.
  These are TOP-LEVEL arrays (not under [settings]).
  Rules are evaluated in order: deny (block) → ask (confirm) → allow (approve).

  Each rule has:
    tool       String (required)   Tool name to match
    pattern    String (optional)   Pattern to match against tool arguments
    reason     String (optional)   Reason shown to LLM (deny) or user (ask)

  Pattern syntax varies by tool:
    shell       — wildcard matching (* = any chars)
    file tools  — glob matching (**, *)
    fetch_url   — domain suffix matching

  Examples:

    [[allow_rules]]
    tool = "shell"
    pattern = "cargo *"

    [[deny_rules]]
    tool = "shell"
    pattern = "rm -rf *"
    reason = "Recursive force deletion is not allowed"

    [[deny_rules]]
    tool = "read_file"
    pattern = ".krew/settings.toml"
    reason = "Config file is protected"

    [[ask_rules]]
    tool = "shell"
    pattern = "npm publish *"
    reason = "Publishing requires confirmation"

─── [providers.<name>] ───────────────────────────────────────────────────────

  type                       "openai" | "anthropic" | "google"   (required)
                             Provider type. "openai" also covers OpenAI-compatible
                             services via base_url.

  api_key                    String
                             API key value (not recommended; prefer api_key_env).

  api_key_env                String
                             Environment variable name holding the API key.

  base_url                   String
                             Custom API endpoint URL. Use for OpenAI-compatible services
                             or self-hosted proxies.

  vertex_project             String
                             Google Vertex AI project ID. Setting this enables Vertex AI
                             mode (google provider only).

  vertex_location            String
                             Google Vertex AI location, e.g. "us-central1"
                             (google provider only).

  extra_headers              Table (key-value pairs)
                             Extra HTTP headers for chat/inference requests.
                             Does not apply to list_models. Do not use header
                             names that conflict with provider-internal or auth
                             headers (Authorization, x-api-key, etc.).

─── [[agents]] (project config only) ─────────────────────────────────────────

  name                       String (required)
                             Unique identifier used for @ addressing.

  display_name               String (required)
                             Human-readable name shown in output.

  provider                   String (required)
                             Provider name, must match a key in [providers.*].

  model                      String (required)
                             LLM model identifier (e.g. "claude-sonnet-4-6").

  api_type                   "chat" | "responses"
                             Default: (none, provider decides)
                             OpenAI only: which API to use.
                               chat      — Chat Completions API
                               responses — Responses API

  color                      String (required)
                             Terminal color for this agent's output.
                             Values: "red", "green", "yellow", "blue", "magenta", "cyan",
                             "white", "gray" (or "grey"), "dark_gray" (or "dark_grey").
                             Unrecognized values fall back to white.

  system_prompt              String
                             Default: (none)
                             Custom system prompt appended to the agent's identity block.

  tools                      Boolean
                             Default: true
                             Whether this agent can use built-in tools.

  enable_web_search          Boolean
                             Default: false
                             Whether to enable the provider's native web search.

  enable_thinking            Boolean
                             Default: false
                             Whether to enable thinking/reasoning mode.

  thinking_effort            "low" | "medium" | "high" | "max"
                             Default: (none)
                             Thinking effort level. Only used when enable_thinking is true.

  [agents.sampling]          (optional sub-table)

    temperature              Float
                             Sampling temperature. OpenAI/Google: 0-2, Anthropic: 0-1.

    top_p                    Float
                             Nucleus sampling probability cutoff (0-1).

    top_k                    Integer
                             Top-K sampling. Only supported by Anthropic and Google.

    max_tokens               Integer
                             Maximum output tokens. Defaults to model maximum.

    frequency_penalty        Float
                             Frequency penalty (-2.0 to 2.0). OpenAI Chat and Google only.

    presence_penalty         Float
                             Presence penalty (-2.0 to 2.0). OpenAI Chat and Google only.

    stop_sequences           Array of strings
                             Stop sequences to halt generation.

─── [[mcp_servers]] ──────────────────────────────────────────────────────────

  MCP (Model Context Protocol) servers provide additional tools to agents.
  Two transport modes are supported:

  name                       String (required)
                             Server name for identification.

  Stdio transport (set command):
    command                  String
                             Command to launch the MCP server process.
    args                     Array of strings
                             Default: []
                             Command-line arguments for the server process.
    env                      Table of string key-value pairs
                             Environment variables passed to the server process.

  HTTP transport (set url):
    url                      String
                             HTTP endpoint URL for Streamable HTTP transport.
    headers                  Table of string key-value pairs
                             HTTP headers sent with every request.

  Common:
    trust                    "auto" | "confirm"
                             Default: "confirm"
                             Trust level controlling tool approval.
                               auto    — skip approval for this server's tools
                               confirm — apply approval_mode rules

─── [skills] ─────────────────────────────────────────────────────────────────

  enabled                    Boolean
                             Default: true
                             Whether the Agent Skills feature is enabled.

  extra_paths                Array of strings
                             Default: []
                             Additional directories to scan for skill files,
                             beyond the default discovery paths.

────────────────────────────────────────────────────────────────────────────────
3. EXAMPLE CONFIGURATIONS
────────────────────────────────────────────────────────────────────────────────

─── User config (~/.krew/settings.toml) ──────────────────────────────────────

  [providers.anthropic]
  type = "anthropic"
  api_key_env = "ANTHROPIC_API_KEY"

  [providers.openai]
  type = "openai"
  api_key_env = "OPENAI_API_KEY"

  [providers.google]
  type = "google"
  api_key_env = "GEMINI_API_KEY"

  [settings]
  approval_mode = "auto-edit"
  language = "Chinese"

─── Project config (.krew/settings.toml) ─────────────────────────────────────

  [settings]
  reply_order = ["claude", "gpt"]

  [[agents]]
  name = "claude"
  display_name = "Claude"
  provider = "anthropic"
  model = "claude-sonnet-4-6"
  color = "blue"
  enable_thinking = true

  [[agents]]
  name = "gpt"
  display_name = "GPT"
  provider = "openai"
  model = "gpt-5.4"
  color = "green"

────────────────────────────────────────────────────────────────────────────────
4. CLI COMMANDS
────────────────────────────────────────────────────────────────────────────────

  krew config init [--user | --project]
      Interactive configuration initialization. Creates config files with
      guided prompts. Use --user for user config only, --project for project
      config only.

  krew config add <provider | agent>
      Add a provider (to user config) or agent (to project config)
      interactively.

  krew config del <provider | agent>
      Delete a provider or agent interactively with confirmation.

  krew config list <providers | agents>
      List configured providers or agents in table format.

  krew config doctor
      Diagnose configuration completeness. Checks config file syntax,
      provider API keys, agent-provider references, and MCP server
      availability.

  krew config help
      Print this configuration manual.

"#;
