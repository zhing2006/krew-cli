# krew-cli — 技术设计文档 (TDD)

> 版本: 0.7.0 | 日期: 2026-03-26
> 参考: [PDD](./PDD.md) | 参考项目: [codex CLI](https://github.com/openai/codex)

---

## 1. 架构总览

### 1.1 系统分层

```txt
┌──────────────────────────────────────────────────┐
│                    CLI Layer                     │
│    (clap 命令解析 + TUI 交互渲染 + Config CLI)     │
├──────────────────────────────────────────────────┤
│                  Session Layer                   │
│       (会话管理 / 消息路由 / @ 寻址解析)           │
├──────────────┬──────────────┬────────────────────┤
│  Agent Loop  │  Tool System │  Slash Commands    │
│  (Agent 循环) │ (工具调度)    │  (命令处理)        │
├──────────────┴──────────────┴────────────────────┤
│               LLM Client Layer                   │
│  (多 Provider 统一抽象: OpenAI/Anthropic/Google/  │
│   Vertex Anthropic/OpenAI-Compatible)             │
├──────────────────────────────────────────────────┤
│              Storage Layer                       │
│         (TOML 文件会话持久化 + 配置管理)            │
└──────────────────────────────────────────────────┘
```

### 1.2 核心模块关系

```txt
                    main.rs
                      │
                      ▼
                ┌──────────┐
                │   App    │──────────┐
                └────┬─────┘          │
                     │                ▼
                     ▼         ┌──────────┐
              ┌──────────┐     │ ConfigMgr│
              │ Session  │     └──────────┘
              └────┬─────┘
                   │
         ┌─────────┼─────────┐
         ▼         ▼         ▼
    ┌─────────┐ ┌─────────┐ ┌─────────┐
    │ Agent   │ │ Agent   │ │ Agent   │
    │ "gpt"   │ │ "opus"  │ │"gemini" │
    └────┬────┘ └────┬────┘ └────┬────┘
         │           │           │
         ▼           ▼           ▼
    ┌─────────┐ ┌─────────┐ ┌─────────┐
    │LLMClient│ │LLMClient│ │LLMClient│
    │ OpenAI  │ │Anthropic│ │ Google  │
    └─────────┘ └─────────┘ └─────────┘
```

---

## 2. 技术选型

### 2.1 语言与运行时

| 选择 | 理由 |
| ---- | ---- |
| **Rust** | 高性能、安全、无 GC，适合 CLI 工具 |
| **tokio** | 异步运行时，处理 Agent 流式响应与 I/O |
| **Edition 2024** | 最新稳定特性集 |

### 2.2 静态链接

产出单文件可执行程序，零外部依赖，五平台均静态链接：

| 平台 | Target Triple | 策略 | 关键配置 |
| ---- | ------------- | ---- | -------- |
| **Windows x64** | `x86_64-pc-windows-msvc` | `static_vcruntime` crate 静态链接 MSVC 运行时 | 在 krew-cli 的 Cargo.toml 中依赖 `static_vcruntime` |
| **Linux x64** | `x86_64-unknown-linux-musl` | musl target 全静态二进制 | `mimalloc` 替换 musl 默认分配器 |
| **Linux arm64** | `aarch64-unknown-linux-musl` | musl target 全静态二进制（cross 交叉编译） | `mimalloc` + `cross` 工具链 |
| **macOS x64** | `x86_64-apple-darwin` | 静态链接 CRT | `RUSTFLAGS="-C target-feature=+crt-static"` |
| **macOS arm64** | `aarch64-apple-darwin` | 静态链接 CRT | `RUSTFLAGS="-C target-feature=+crt-static"` |

**Release 优化**：`[profile.release]` 启用 `lto = true`、`codegen-units = 1`、`strip = true`、`panic = "abort"`。

**TLS 依赖**：reqwest 0.13 默认使用 rustls，无需额外配置。关闭 `default-features` 后通过 `rustls` feature 显式启用，避免 musl 下 OpenSSL 静态编译问题。

**分发方式**：

- **GitHub Release**：GitHub Actions 在 `v*` tag push 时自动构建五平台二进制并创建 Release
- **npm**：`npm install -g @zhing2026/krew`，使用 optionalDependencies 平台子包模式（`@zhing2026/krew-{platform}`）

### 2.3 关键 Crate

所有 crate 使用最新稳定版，`default-features = false` 最小化依赖，通过 workspace 统一管理避免 dup crates。

| Crate | 用途 | 版本 |
| ----- | ---- | ---- |
| `clap` | CLI 参数解析 (derive) | 4 |
| `tokio` | 异步运行时 | 1 |
| `ratatui` | TUI 渲染 | 0.30 |
| `crossterm` | 终端事件处理（event-stream、bracketed-paste） | 0.29 |
| `reqwest` | HTTP 客户端（LLM API，默认 rustls） | 0.13 |
| `eventsource-stream` | SSE 流式响应解析（从 reqwest bytes_stream 转换） | 0.2 |
| `serde` | 序列化框架 | 1 |
| `serde_json` | JSON 序列化 | 1 |
| `toml` | TOML 配置/会话文件序列化 | 1.1 |
| `toml_edit` | 格式保留 TOML 编辑（config writer） | 0.25 |
| `dialoguer` | 交互式终端对话框（config wizard） | 0.12 |
| `syntect` | 代码语法高亮 | 5 |
| `anyhow` | 应用层错误处理 | 1 |
| `thiserror` | 库层错误类型定义 | 2 |
| `tracing` | 日志/追踪 | 0.1 |
| `tracing-subscriber` | 日志输出 | 0.3 |
| `uuid` | Session ID 生成 | 1 |
| `chrono` | 时间戳处理 | 0.4 |
| `futures` | Stream trait 等异步原语 | 0.3 |
| `async-trait` | 异步 trait 支持 | 0.1 |
| `pulldown-cmark` | Markdown 解析 | 0.13 |
| `two-face` | syntect 语法/主题捆绑包 | 0.5 |
| `similar` | 文本差异比较 | 2 |
| `diffy` | Diff 生成与格式化 | 0.4 |
| `unicode-width` | Unicode 字符宽度计算 | 0.2 |
| `unicode-segmentation` | Unicode 字素分割 | 1 |
| `textwrap` | 文本自动换行 | 0.16 |
| `dunce` | 路径标准化（Windows UNC 路径处理） | 1 |
| `walkdir` | 目录遍历 | 2 |
| `globset` | Glob 模式匹配 | 0.4 |
| `regex` | 正则表达式引擎 | 1 |
| `http` | HTTP 类型定义 | 1 |
| `rmcp` | MCP SDK（child-process + streamable-http） | 1 |
| `htmd` | HTML 转 Markdown | 0.5 |
| `tracing-appender` | 日志文件输出（按日滚动） | 0.2 |
| `static_vcruntime` | Windows MSVC 运行时静态链接（仅 Windows） | 3 |
| `mimalloc` | 全局分配器（musl 性能优化，仅 Linux） | 0.1 |

---

## 3. 核心模块设计

### 3.1 Agent Loop

参考 codex 的事件驱动模式，简化为：

```txt
用户输入
    │
    ▼
解析 @ 寻址 ──→ 确定目标 Agent 列表
    │
    ▼
构建消息(Message) ──→ 追加到 Session.messages
    │
    ▼
按 reply_order 串行向目标 Agent 发起 LLM 请求
    │
    ▼
┌───────────── 每个 Agent 的处理循环 ─────────────┐
│                                                │
│  接收流式响应 ──→ 检查是否有 tool_call           │
│       │                    │                   │
│       │ 无                 │ 有                │
│       ▼                    ▼                   │
│  流式输出文本         执行工具调用               │
│       │              (可能需用户审批)           │
│       │                    │                   │
│       │                    ▼                   │
│       │              将工具结果追加到消息        │
│       │              重新请求 LLM（工具循环）    │
│       │                    │                   │
│       ▼                    ▼                   │
│     Agent 回合结束                              │
└────────────────────────────────────────────────┘
    │
    ▼
将 Agent 回复追加到 Session.messages
    │
    ▼
等待下一次用户输入
```

#### 3.1.1 串行执行顺序

当 `@all` 时，按 `reply_order` 配置顺序**串行**执行每个 Agent 的完整 Agent Loop。前一个 Agent 跑完（包括所有工具调用）后，其回复追加到共享消息历史，下一个 Agent 才开始。这确保：

- 后续 Agent 能看到前面 Agent 的回复
- 工具调用不会冲突
- 用户审批流程清晰

```rust
// 伪代码
for agent_name in &config.settings.reply_order {
    let agent = agents.get(agent_name);
    // 每个 Agent 看到完整历史（包括前面 Agent 的回复）
    let response = agent.run_loop(&session.messages, &tools).await?;
    renderer.stream_output(agent_name, &response).await;
    session.messages.push(response.to_message());
    storage.save_message(session_id, &response).await?;
}
```

#### 3.1.2 Agent Identity Prompt

`build_identity_prompt()` 构建每个 Agent 的身份描述块，作为 system prompt 的一部分发送给 LLM。内容按顺序包含：

1. **Agent 身份**：display_name、model、agent_name
2. **krew-cli 简介**：说明 krew-cli 是一个多 AI Agent 协作 CLI 工具，用户在一个终端中同时与多个 LLM 对话
3. **配置帮助提示**：告知 Agent 当需要帮助用户修改 krew 配置时，可执行 `krew config help` 获取完整配置手册
4. **多 Agent 对话规则**：其他 Agent 消息前缀、不要模仿其他 Agent
5. **当前日期时间**
6. **语言指令**（如 `settings.language` 已配置）
7. **Peer Agent 协作提示**（如有密语目标 Agent）
8. **Whisper 隐私上下文**（如有密语）

#### 3.1.3 AgentEvent 通信协议

Agent Loop 通过 channel 发送 `AgentEvent` 与 TUI 层通信：

```rust
enum AgentEvent {
    /// Agent 开始回复
    ResponseStart { agent_name: String, display_name: String, color: String },
    /// 文本 token
    TextDelta(String),
    /// 思考/推理内容
    ThinkingDelta(String),
    /// 服务端工具开始（如 Web Search）
    ServerToolStart { name: String },
    /// 服务端工具完成
    ServerToolDone { name: String, query: Option<String> },
    /// 工具调用开始
    ToolCallStart { name: String, arguments: String },
    /// 工具实时输出（如 shell 流式输出）
    ToolCallOutput { text: String },
    /// 工具调用完成
    ToolCallDone { name: String, result_summary: String },
    /// 回复完成，携带 token 用量和中间消息
    Done { usage: Usage, intermediate_messages: Vec<ChatMessage>, final_text: String,
           server_tool_uses: Vec<ServerToolUseInfo> },
    /// 错误
    Error { message: String, intermediate_messages: Vec<ChatMessage> },
    /// 需要用户审批
    ApprovalRequest { tool_name: String, arguments: String,
                      allow_session_approval: bool,
                      respond: oneshot::Sender<ReviewDecision> },
    /// 正在重试（429/5xx）
    Retrying { attempt: u32, max_attempts: u32, reason: String, delay_secs: f64 },
}
```

#### 3.1.4 工具调用循环

当 Agent 回复包含 ToolCall 时，进入工具调用循环：

1. 收集当前轮次所有 ToolCall
2. 检查审批策略（不需审批的并行执行，需审批的依次发送 ApprovalRequest）
3. 执行工具，追加 Tool role 消息（含 tool_call_id 和结果）到消息历史
4. 再次调用 LLM（携带工具结果）
5. 重复直到无 ToolCall 或达到最大轮数（默认 25 轮）

### 3.2 消息路由（@ 寻址）

#### 3.2.1 寻址解析

```rust
enum Addressee {
    All,                     // @all (anywhere in input)
    Single(String),          // @name (single known agent)
    Multiple(Vec<String>),   // @gpt @opus (multiple known agents)
    LastRespondent,          // no recognized @ token
}

/// Parse user input into an addressee and message body.
///
/// `@name` tokens are recognized anywhere in the input (not just prefix),
/// but only if `name` matches a known agent or `"all"`. Unrecognized
/// `@tokens` (including bare `@`) are treated as plain text.
///
/// The message body is always the **full original input** — `@name` tokens
/// are not stripped, preserving context for the LLM.
fn parse_input(input: &str, known_agents: &[String]) -> Result<(Addressee, String, bool)> {
    let input = input.trim();
    if input.is_empty() {
        return Err(anyhow!("empty input"));
    }

    // Scan all whitespace-delimited words for @name and #name tokens.
    let mut at_matched: Vec<String> = Vec::new();
    let mut hash_matched: Vec<String> = Vec::new();
    for word in input.split_whitespace() {
        if let Some(name) = word.strip_prefix('@') { /* ... */ }
        else if let Some(name) = word.strip_prefix('#') { /* ... */ }
    }

    // Reject mixing @ and #, reject #all.
    let is_whisper = !hash_matched.is_empty();
    let matched = if is_whisper { hash_matched } else { at_matched };

    if matched.is_empty() {
        Ok((Addressee::LastRespondent, message, false))
    } else if matched.iter().any(|n| n == "all") {
        Ok((Addressee::All, message, false))
    } else if matched.len() == 1 {
        Ok((Addressee::Single(matched[0].clone()), message, is_whisper))
    } else {
        Ok((Addressee::Multiple(matched), message, is_whisper))
    }
}
```

**边界情况处理：**

- `hey @gpt what do you think` → `Addressee::Single("gpt")` + 原始完整输入（`@name` 可出现在任意位置）
- `@gpt @opus 你们觉得呢` → `Addressee::Multiple(["gpt", "opus"])` + 原始完整输入
- `@all @opus 混合` → `Addressee::All`（`@all` 优先级最高）
- `@unknown 你好` → `Addressee::LastRespondent`（未知 agent 名被忽略）
- `@`（裸 @）→ 当作普通文本，`Addressee::LastRespondent`
- 空输入 → 报错
- `#opus hello` → `Addressee::Single("opus")`, `is_whisper = true`
- `#opus #gemini discuss` → `Addressee::Multiple(["opus", "gemini"])`, `is_whisper = true`
- `#all hello` → 报错（禁止）
- `@gpt #opus hello` → 报错（`@` 和 `#` 不可混用）

#### 3.2.2 路由规则

| 寻址 | 接收 LLM 请求的 Agent | 消息可见性 |
| ---- | -------------------- | --------- |
| `@all` | 所有 Agent（按 reply_order 串行） | 全部可见 |
| `@gpt` | 仅 gpt | 全部可见（上下文共享） |
| `@gpt @opus` | gpt 和 opus（按 @ 出现顺序串行） | 全部可见 |
| 无识别的 @/# | 上一个回答者；无则使用 `reply_order` 中第一个可用的 Agent | 全部可见 |
| `#opus` | 仅 opus | 仅 opus 可见内容，其他 Agent 看到占位符 |
| `#opus #gemini` | opus 和 gemini | 组内成员互相可见，组外 Agent 看到占位符 |

### 3.3 LLM Client 抽象层

#### 3.3.1 统一 Trait

```rust
#[async_trait]
trait LlmClient: Send + Sync {
    /// 发起流式对话请求
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolSpec],
        sampling: &SamplingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent>>>>;
}

enum StreamEvent {
    /// 文本 token
    TextDelta(String),
    /// 工具调用请求
    ToolCall {
        id: String,
        name: String,
        arguments: String,
    },
    /// 思考/推理内容
    ThinkingDelta(String),
    /// 服务端工具开始（如 Web Search）
    ServerToolStart { name: String },
    /// 服务端工具完成
    ServerToolDone { name: String, query: Option<String> },
    /// 流结束，携带本次请求的 token 用量
    Done(Usage),
    /// 错误
    Error(String),
}

/// 各 Client 在流结束时统一产出的 token 用量。
/// 各 Provider 原始字段名和返回时机不同（见 §3.5.3 映射表），由各 Client 实现统一收集并映射为此结构。
/// 若 Provider 未直接返回 total_tokens（如 Anthropic），由 Client 计算:
///   total_tokens = prompt_tokens + completion_tokens
struct Usage {
    prompt_tokens: u32,         // 输入 token 数
    completion_tokens: u32,     // 输出 token 数（含推理 tokens）
    total_tokens: u32,          // 总计（部分 Provider 需由 Client 端计算）
}
```

#### 3.3.2 Provider 实现

所有 provider 支持可选的 `extra_headers` 配置（`ProviderConfig.extra_headers`），额外 HTTP headers 仅应用于 chat/inference 请求（`chat_stream()` → `send_with_retry()`），不影响 `list_models` 等非推理请求。用户不应配置与 provider 内部或认证 headers 冲突的名称。

##### OpenAI Client

同时支持两种 API，通过 Agent 配置的 `api_type` 字段选择：

- **Responses API** (`api_type = "responses"`)
  - API: `POST /v1/responses` (stream=true)
  - 请求格式: `{ model, input: [...], tools: [...], stream: true }`
  - 响应事件: `response.output_item.added`, `response.output_text.delta`, `response.completed` 等
  - Web Search: tools 中添加 `{ type: "web_search" }`
  - Thinking: `reasoning` 字段 `{ effort: "low"|"medium"|"high", summary: "auto" }`
- **Chat Completions API** (`api_type = "chat"`)
  - API: `POST /v1/chat/completions` (stream=true, stream_options.include_usage=true)
  - 请求格式: `{ model, messages: [...], tools: [...], stream: true }`
  - 响应事件: 标准 SSE `data: {"choices":[{"delta":...}]}`
  - Web Search: 通过 `web_search_options` 支持（OpenAI 原生 + LiteLLM 代理）
  - 支持自定义 `base_url`（用于 LiteLLM 等代理）
  - 连续相同 role 消息自动合并

##### Anthropic Client

- API: `POST /v1/messages` (stream=true)
- 使用 `x-api-key` header 认证（非 Bearer）
- SSE 解析流式响应（`content_block_delta`、`message_delta`）
- 消息格式: `{ role, content: [{ type: "text" | "tool_use" | "tool_result" }] }`
- system 消息分离到顶层 `system` 字段
- `max_tokens` **必填**，未设置时根据模型名自动填入最大输出 token 数
- `temperature` clamp 到 0-1 范围
- Web Search: tools 中添加 `{ type: "web_search_20250305", name: "web_search" }`
- Thinking: 使用能力函数矩阵判断模型支持。Opus 4.6/Sonnet 4.6 使用 `adaptive` thinking + `output_config.effort`（含 max）；Opus 4.5 使用 `enabled` + `budget_tokens` + `output_config.effort`（Max 降为 high）；其他旧模型仅使用 `enabled` + `budget_tokens`，不发送 effort。启用 thinking 时强制 `temperature = 1.0`

##### Vertex Anthropic Client

- API: Vertex AI `publishers/anthropic/models/{model}:streamRawPredict` (stream=true)
- 使用 `Authorization: Bearer <token>` 认证；`api_key` / `api_key_env` 可以是 Google OAuth access token 或 LiteLLM virtual key
- 请求体复用 Anthropic Messages 字段，但 `model` 在 URL 中，body 顶层不发送 `model`
- body 顶层发送 `anthropic_version = "vertex-2023-10-16"`
- 复用 Anthropic 的 message conversion、tool conversion、thinking/output_config、sampling 参数和 SSE parser
- 支持 Google 官方 Vertex endpoint 和 LiteLLM Vertex passthrough root（如 `/vertex_ai`、`/vertex_ai/v1`）
- Web Search: tools 中添加 `{ type: "web_search_20250305", name: "web_search" }`
- `vertex_project` 和 `vertex_location` 运行时必填；缺失时 agent 初始化跳过并给出 warning
- `extra_headers`: 支持 provider 级别的自定义 HTTP headers（仅 chat/inference 请求）

Vertex Anthropic host 选择：

| location | host |
| -------- | ---- |
| `global` | `aiplatform.googleapis.com` |
| `us` | `aiplatform.us.rep.googleapis.com` |
| `eu` | `aiplatform.eu.rep.googleapis.com` |
| 其他 region | `{location}-aiplatform.googleapis.com` |

Vertex Anthropic URL 拼接：

| base_url | 请求路径 |
| -------- | -------- |
| 未设置 | `https://{host}/v1/projects/{project}/locations/{location}/publishers/anthropic/models/{model}:streamRawPredict` |
| `https://litellm.example.com/vertex_ai` | `https://litellm.example.com/vertex_ai/v1/projects/...` |
| `https://litellm.example.com/vertex_ai/v1` | `https://litellm.example.com/vertex_ai/v1/projects/...` |
| `https://proxy.example.com` | `https://proxy.example.com/v1/projects/...` |

##### Google Client

- API: Gemini `/models/{model}:streamGenerateContent` (stream=true)
- **Vertex AI 模式**：设置 `vertex_project`/`vertex_location` 时使用 Bearer token 认证
- SSE 解析（data-only SSE）
- 消息格式: `{ role: "user"|"model", parts: [{ text } | { functionCall }] }`
- system 消息分离到 `systemInstruction` 字段
- Web Search: tools 中添加 `{ google_search: {} }`
- Thinking: Gemini 3.x 使用 `thinkingConfig.thinkingLevel`；Gemini 2.5 使用 `thinkingConfig.thinkingBudget`
- `extra_headers`: 支持 provider 级别的自定义 HTTP headers（仅 chat/inference 请求），用于 Vertex AI Priority PayGo 等场景

##### OpenAI-Compatible Client

- 复用 OpenAI Chat Completions 实现，替换 base_url 和认证方式
- 用于接入豆包（ByteDance）等第三方 OpenAI 兼容服务
- 同样支持 `api_type` 配置（默认 `chat`）；config wizard 中 OpenAI 兼容 Provider 默认选择 Chat Completions API
- 如果 `base_url` 以 `/v1` 结尾，自动去掉以避免请求路径出现 `/v1/v1/...`
- Web Search: 取决于具体服务是否支持

#### 3.3.3 消息格式转换

每个 Provider 实现中包含一个 `convert_messages` 方法，将统一的 `ChatMessage` 转换为各 Provider 特定的 API 请求格式。**转换时需要感知"当前是哪个 Agent"**，以正确设置 role。

**核心问题：其他 Agent 的回复用什么 role 发送？**

对于 Agent "opus" 视角下的消息历史：

| 原始消息 | 发送给 opus 时的 role | 处理方式 |
| -------- | -------------------- | -------- |
| 用户消息 | `user` | 直接发送 |
| opus 自己之前的回复 | `assistant` | 直接发送 |
| gpt 的回复 | 由 `other_agent_role` 配置决定 | 方案 A 或 B |

**方案 A：其他 Agent 的回复作为 `user` role**（默认）

```txt
{ role: "user", content: "[gpt] GPT-5.2:\n我建议使用 VecDeque..." }
```

优点：LLM 不会混淆自己和别人的发言。

**方案 B：其他 Agent 的回复作为 `assistant` role**

```txt
{ role: "assistant", content: "[gpt] GPT-5.2:\n我建议使用 VecDeque..." }
```

优点：LLM 知道这是 AI 级别的回复。

> **决策：通过 `settings.other_agent_role` 配置项控制。** 默认使用方案 A（`user`），可切换为方案 B（`assistant`）。`convert_messages` 方法接收 `self_agent_name` 和 `other_agent_role` 参数。

```rust
enum OtherAgentRole {
    User,       // 方案 A（默认）
    Assistant,  // 方案 B
}
```

### 3.3.4 错误处理与重试

LLM API 调用的错误处理策略，通过 `settings.retry` 可配置：

```rust
struct RetryConfig {
    max_retries_rate_limit: u32,       // 429 重试次数，默认 3
    max_retries_server_error: u32,     // 5xx 重试次数，默认 2
    backoff_base_secs: f64,            // 退避基数（秒），默认 2.0
    backoff_multiplier: f64,           // 退避倍数，默认 3.0
    server_error_interval_secs: f64,   // 5xx 重试间隔（秒），默认 2.0
    request_timeout_secs: u64,         // 请求超时（秒），默认 60
}
```

| 错误类型 | 处理方式 |
| -------- | -------- |
| 429 Rate Limit | 指数退避重试，最多 `max_retries_rate_limit` 次 |
| 5xx 服务端错误 | 固定间隔重试 `max_retries_server_error` 次 |
| 网络超时 | 超时阈值 `request_timeout_secs`，超时后重试 1 次 |
| 网络断开 | 提示用户，等待恢复后可手动重试（输入相同消息） |
| 401/403 认证错误 | 不重试，直接报错提示检查 API Key |
| 流式中断 | 已接收的部分内容保留显示，提示用户该 Agent 回复不完整 |

**@all 场景**：某个 Agent 失败不影响其他 Agent 的执行，失败的 Agent 显示错误信息后跳过，流程继续下一个 Agent。

### 3.3.5 原生 Web Search

当 Agent 的 `enable_web_search = true` 时，LLM Client 在请求中注入对应 Provider 的原生搜索工具。各 Provider 实现不同：

| Provider | 注入方式 | 工具标识 |
| -------- | -------- | -------- |
| OpenAI (Responses API) | `tools` 数组加 `{ type: "web_search" }` | `web_search` |
| OpenAI (Chat Completions) | `web_search_options: { search_context_size: "medium" }` | `web_search` |
| Anthropic | `tools` 数组加 `{ type: "web_search_20250305", name: "web_search" }` | `web_search_20250305` |
| Vertex Anthropic | `tools` 数组加 `{ type: "web_search_20250305", name: "web_search" }` | `web_search_20250305` |
| Google Gemini | `tools` 数组加 `{ google_search: {} }` | `google_search` |
| OpenAI-Compatible | 取决于服务商是否支持 | - |

搜索由模型自主决定是否触发（非每次必搜），搜索结果中的引用信息在终端输出中简化显示为 `[n]` 脚注格式。

### 3.3.6 采样参数映射

`SamplingConfig` 中的统一字段到各 Provider API 请求字段的映射关系：

| 统一字段 | OpenAI Chat Completions | OpenAI Responses | Anthropic Messages | Google Gemini |
| -------- | ---------------------- | ---------------- | ------------------ | ------------- |
| `temperature` | `temperature` (0-2) | `temperature` (0-2) | `temperature` (0-1, 超出范围 clamp) | `generationConfig.temperature` (0-2) |
| `top_p` | `top_p` | `top_p` | `top_p` | `generationConfig.topP` |
| `top_k` | — (忽略) | — (忽略) | `top_k` | `generationConfig.topK` |
| `max_tokens` | `max_completion_tokens` | `max_output_tokens` | `max_tokens` (**必填**，未设置时取模型最大值) | `generationConfig.maxOutputTokens` |
| `frequency_penalty` | `frequency_penalty` | — (忽略) | — (忽略) | `generationConfig.frequencyPenalty` |
| `presence_penalty` | `presence_penalty` | — (忽略) | — (忽略) | `generationConfig.presencePenalty` |
| `stop_sequences` | `stop` | — (忽略) | `stop_sequences` | `generationConfig.stopSequences` |

**默认值策略：**

- `max_tokens`：默认取各模型的最大输出 token 数，确保不截断长回复。各模型参考值：

| 模型 | 最大输出 Tokens |
| ---- | -------------- |
| GPT-4o | 16,384 |
| GPT-4.1 / 4.1-mini / 4.1-nano | 32,768 |
| o3 / o4-mini | 100,000 |
| Claude Opus 4 / 4.5 / 4.6 | 32,000 |
| Claude Sonnet 4 / 4.5 / 4.6 | 64,000 |
| Claude Haiku 4.5 | 64,000 |
| Gemini 2.5 Pro / Flash | 65,536 |

- `temperature`：默认不设置（各 Provider 默认 1.0），用户可按场景自行调整
- 其余参数：默认不设置，使用 Provider 默认值

**Anthropic 特殊处理：** Anthropic 的 `max_tokens` 为必填参数，当用户未配置 `sampling.max_tokens` 时，LLM Client 必须根据模型名称自动填入对应的最大输出 token 数。

### 3.3.7 Thinking 配置映射

各 Provider 的 thinking/reasoning 参数映射：

| Provider | 模型 | API 参数 | effort 映射 |
| -------- | ---- | -------- | ----------- |
| OpenAI Responses | 白名单模型 (gpt-5.5/5.5-pro/5.4/5.4-pro/5.3-codex/5.2) | `reasoning: { effort, summary: "auto" }` | low→"low", medium→"medium", high→"high", max→"xhigh", 未设置→"medium" |
| OpenAI Responses | 其他模型 | `reasoning: { effort, summary: "auto" }` | low→"low", medium→"medium", high→"high", max→"high" (降级), 未设置→"medium" |
| Anthropic | Opus 4.6 / Sonnet 4.6 | `thinking: { type: "adaptive" }` + `output_config: { effort }` | 使用 adaptive thinking, effort 含 max |
| Anthropic | Opus 4.5 | `thinking: { type: "enabled", budget_tokens }` + `output_config: { effort }` | budget: low→1024, medium→8192, high/max→32768; effort: max 降为 "high" |
| Anthropic | 其他旧模型 | `thinking: { type: "enabled", budget_tokens }` | low→1024, medium→8192, high/max→32768, 未设置→8192, 不发送 effort |
| Google | Gemini 3.x | `thinkingConfig: { includeThoughts: true, thinkingLevel }` | low→"low", medium→"medium", high/max→"high", 未设置→"high" |
| Google | Gemini 2.5 | `thinkingConfig: { includeThoughts: true, thinkingBudget }` | low→1024, medium→8192, high/max→24576, 未设置→-1 (动态) |

**Anthropic 强制约束**：启用 thinking 时强制 `temperature = 1.0`，覆盖用户配置（记录 warn 日志）。

### 3.3.8 通用工具函数（llm-common）

`krew-llm::common` 模块提供各 Provider 共享的通用函数：

- **HTTP 状态码分类**：区分 429 / 5xx / 认证错误
- **错误消息提取**：从 HTTP 响应 body 提取错误信息
- **带重试请求发送**：统一的重试逻辑（429 指数退避、5xx 固定间隔、超时重试）
- **连续相同 Role 消息合并**：合并连续的 user 或 assistant 消息（部分 Provider 要求 role 交替）

### 3.3.9 安全边界

**路径边界**：内置文件工具（read_file, write_file, edit_file, glob, grep）在执行前校验路径：

- 解析后的绝对路径必须在 `session.cwd` 及其子目录内
- 拒绝包含 `..` 的路径穿越尝试
- 符号链接解析后仍需在边界内

**MCP 信任级别**：McpServerConfig 的 `trust` 字段（见 3.7.2 配置数据结构）控制该 MCP server 的审批行为。`trust = "auto"` 时跳过审批，`trust = "confirm"`（默认）时按 approval_mode 规则确认。

### 3.4 工具系统

#### 3.4.1 工具抽象

工具系统使用 `ToolHandler` trait（执行逻辑）+ `ToolSpec` 结构体（元数据）双层设计：

```rust
/// Tool execution handler.
trait ToolHandler: Send + Sync {
    fn name(&self) -> &str;
    fn requires_approval(&self) -> bool;
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult>;
}

/// Tool metadata (name, description, JSON Schema).
struct ToolSpec {
    name: String,
    description: String,
    parameters_schema: serde_json::Value,
}

struct ToolResult {
    content: String,
    is_error: bool,
    images: Vec<ImageContent>,  // image data from read_file on image files
}

struct ImageContent {
    data: Vec<u8>,              // raw image bytes
    media_type: String,         // MIME type (e.g. "image/png")
    filename: Option<String>,   // original filename
}
```

#### 3.4.2 工具注册中心

`ToolRegistry` 统一管理工具的注册与查询：

```rust
impl ToolRegistry {
    /// Create registry with read-only tools (read_file, glob, grep).
    fn create_readonly_registry(cwd: PathBuf, restrict_workspace: bool) -> ToolRegistry;

    /// Create registry with all 7 built-in tools + activate_skill (when skills non-empty).
    fn create_full_registry(cwd: PathBuf, restrict_workspace: bool, skills: HashMap<String, SkillInfo>) -> ToolRegistry;

    /// Dynamically register a tool (used for MCP tools and activate_skill).
    fn register(&mut self, spec: ToolSpec, handler: Arc<dyn ToolHandler>);

    /// Query whether a tool requires approval.
    fn requires_approval(&self, name: &str) -> bool;
}
```

#### 3.4.3 内置工具列表

| 工具名 | 功能 | 类别 | suggest 下 | auto-edit 下 | full-auto 下 |
| ------ | ---- | ---- | --------- | ----------- | ----------- |
| `read_file` | 读取文件内容（带行号，支持 offset/limit）；支持图片文件（png/jpg/jpeg/gif/webp），自动识别并返回图片数据，上限 20MB | 读操作 | 自动 | 自动 | 自动 |
| `write_file` | 写入/创建文件（自动创建父目录） | 写操作 | 需确认 | 自动 | 自动 |
| `edit_file` | 搜索替换编辑（验证唯一匹配，生成 unified diff） | 写操作 | 需确认 | 自动 | 自动 |
| `shell` | 执行 Shell 命令（默认超时 120s，输出限制 100KB） | Shell | 需确认* | 需确认* | 自动 |
| `glob` | 文件名模式匹配（globset + walkdir） | 读操作 | 自动 | 自动 | 自动 |
| `grep` | 文件内容搜索（正则匹配，支持 include 过滤） | 读操作 | 自动 | 自动 | 自动 |
| `fetch_url` | 抓取 URL 内容（HTTP→HTTPS 升级，HTML→Markdown，1MB 限制） | 网络 | 白名单自动，其他需确认 | 同左 | 自动 |
| `activate_skill` | 激活 Skill，加载完整指令（发现 Skills 时自动注册） | 读操作 | 自动 | 自动 | 自动 |

工具审批通过 `[[allow_rules]]`、`[[deny_rules]]`、`[[ask_rules]]` 权限规则进行细粒度控制。保护路径（`.git/`、`.krew/`、`.env` 等）即使在 `full-auto` 模式下也始终受保护。

**Shell 工具细节：**

- **跨平台 Shell 检测**：Windows 优先 Git Bash（搜索顺序：`KREW_BASH_PATH` 环境变量 → PATH 中的 bash.exe（跳过 WSL bash）→ `C:\Program Files\Git\bin\bash.exe`）；Unix 使用 `$SHELL` → `/bin/sh`
- **超时**：`timeout_seconds` 参数（默认 120 秒）
- **输出截断**：超过 100KB 时截断并附带提示 `[output truncated at 100KB]`
- **Windows**：使用 `CREATE_NO_WINDOW` 创建标志防止控制台窗口闪现

**fetch_url 工具细节：**

- HTTP URL 自动升级为 HTTPS
- 响应大小限制 1MB，超出截断
- 域名匹配通过 `[[allow_rules]]` 中 `tool = "fetch_url"` 配置：后缀匹配（如 `docs.github.com` 匹配 `github.com`）
- 跟随重定向，超时 30 秒

**read_file 图片输入：**

`read_file` 工具支持读取图片文件（png/jpg/jpeg/gif/webp），实现多模态输入：

- **扩展名检测**：在 `check_binary()` 之前按扩展名判断，匹配到图片格式时跳过 binary 检测
- **大小限制**：图片专用上限 `MAX_IMAGE_SIZE = 20MB`（低于文本文件的 100MB），因图片数据会常驻 messages 历史
- **数据传递**：`ToolResult.images` → `agent_loop` 映射为 `ChatMessage.images`（`krew-tools` 与 `krew-llm` 各自定义 `ImageContent`，由 `krew-core` 逐字段映射）
- **Provider 序列化**：各 provider 在 `convert_messages()` 中将图片 bytes 编码为 base64 并按各自 API 格式序列化
  - Anthropic：`content` 数组中 `type: "image"` + `source.type: "base64"`
  - Google Gemini：`functionResponse.parts[].inlineData`（含 `displayName` + `$ref` 引用）
  - OpenAI Responses：`output` 数组中 `type: "input_image"` + data URI
  - OpenAI Chat Completions：降级为纯文本（该 API 不支持 tool result 带图片）
- **持久化**：`ChatMessage.images` 标记 `#[serde(skip)]`，不写入 session 文件
- **跨 Agent 可见性**：仅调用 `read_file` 的 Agent 能看到图片数据；其他 Agent 看到的是折叠后的文本摘要 `[Image: filename]`

#### 3.4.4 MCP 集成

MCP 实现分为三个模块：

- `McpClient` — 底层通信，封装 rmcp SDK，支持 stdio 和 Streamable HTTP 两种传输
- `McpManager` — 管理多个 MCP 服务器的生命周期，提供统一的工具发现和调用接口。工具使用限定名格式 `mcp__{server}__{tool}`（字符清理），显示名格式 `mcp:{server}/{tool}`
- `McpToolHandler` — 将 MCP 工具适配为 ToolHandler trait，统一注册到 ToolRegistry

**双传输支持**：

```toml
# Stdio 传输（子进程）
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]

# HTTP 传输（Streamable HTTP）
[[mcp_servers]]
name = "remote-tools"
url = "https://mcp.example.com/sse"
headers = { Authorization = "Bearer $TOKEN" }
```

MCP 服务器在会话启动时初始化，发现的工具自动注册到 ToolRegistry 中，与内置工具统一暴露给 Agent。工具的审批级别由 MCP 服务器的 `trust` 配置和工具的 `annotations` 元数据共同决定：

- `trust = "auto"` → `requires_approval()` 始终返回 false
- `trust = "confirm"`（默认）→ 根据 annotations 判断（`read_only` 且非 `destructive` 可自动执行，否则需确认）

环境变量值支持 `$VAR` 语法展开。

#### 3.4.5 工具审批流程

```rust
enum ReviewDecision {
    Approved,              // 本次批准
    ApprovedForSession,    // 本次会话内按策略缓存（shell 按前缀、fetch_url 按 host、其他按工具名）
    Denied,                // 拒绝
    Abort,                 // 中止整个 Agent 回合
}
```

```txt
Agent 请求工具调用
    │
    ▼
检查审批策略(approval_mode) + 工具类型
    │
    │  读操作(read_file/glob/grep/activate_skill): 所有策略下均自动执行
    │
    ├── full-auto ──→ 所有工具直接执行
    │
    ├── auto-edit ──→ 写操作(write_file/edit_file)自动执行
    │                  Shell/MCP 需确认
    │
    └── suggest ───→ 写操作/Shell/MCP 均需确认
                          │
                          ▼
                    渲染审批 Overlay（含工具名、参数、diff 预览）
                    快捷键: y=批准 / a=会话级批准 / n=拒绝 / Ctrl+C=中止
                          │
                          ▼
                    用户选择 ──→ 执行或跳过
```

**会话级审批缓存**：选择 `ApprovedForSession` 后，按工具类型分策略缓存：

- **Shell 工具**：按命令前缀缓存（如 `cargo build`），同前缀不同参数自动放行，不同子命令仍需审批
- **fetch_url**：按域名缓存，同 host 不同路径自动放行，不同域名仍需审批
- **其他写工具/MCP 工具**：按工具名缓存，同名工具后续调用自动放行

**审批 Overlay 细节：**

- 布局：工具调用信息 + 可选项列表（支持 `↑`/`↓` 导航、`Enter` 确认）
- 快捷键：`y`=批准 / `a`=会话级批准 / `n` 或 `Esc`=拒绝 / `Ctrl+C`=中止
- `edit_file` 审批显示 colored unified diff 预览
- `write_file` 审批显示文件内容预览
- 多个工具调用串行处理（审批队列，逐一显示）

### 3.5 Slash 命令系统

```rust
enum SlashCommand {
    Clear,              // 清屏（同 /new）
    Resume,             // 恢复历史会话
    Rewind,             // 回退到历史消息（fork 语义）
    Agents,             // 列出 Agent 及 token 用量
    Compact(String),    // 参数: agent name
    Mcp,                // 列出 MCP 服务器及工具
    Skills,             // 列出可用技能
    Stats,              // 显示进程统计（内存、线程）
    Help,
    Exit,               // 退出（同 /quit）
}

impl SlashCommand {
    fn from_input(input: &str) -> Option<SlashCommand>;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, ctx: &mut AppContext) -> Result<()>;
}
```

命令发现：输入 `/` 后，匹配所有内置命令和自定义命令名前缀，弹出补全列表。内置命令优先级高于自定义命令。

#### 3.5.1 /agents 输出规格

`/agents` 命令输出当前会话的 Agent 列表及 token 用量统计。

**输出格式：**

```txt
Agents in session:
  [gpt]    GPT-5.5          openai/gpt-5.5           3,284 tokens (1,250 in / 2,034 out)
  [opus]   Claude Opus      anthropic/claude-opus-4-6 5,642 tokens (3,512 in / 2,130 out)
──────────────────────────────────────────────────────
  Total: 8,926 tokens
```

**聚合规则：**

- 遍历 `session.messages` 中所有 `role = Assistant` 的消息，按 `name` 字段分组
- 每个 Agent 累加其所有消息的 `usage.prompt_tokens` 和 `usage.completion_tokens`
- Total 行使用 `session.total_tokens_used`

#### 3.5.2 /compact 实现方案

`/compact <agent_name>` 使用指定 Agent 将当前会话历史压缩为一段摘要。

**流程：**

```txt
1. 取 session.messages 中除最后 N 轮外的所有消息作为"待压缩区"
   （N 由 compact_keep_rounds 配置，默认 10 轮。一轮 = 一条用户消息 + 其后所有非用户消息）
2. 从待压缩区提取密语消息（带 whisper_targets 的用户消息及其 agent 回复）
3. 从待压缩区提取 skill activation 消息
4. 构建压缩请求: system="请将以下对话历史压缩为简洁摘要" + 剩余待压缩区消息
5. 调用指定 Agent 的 LLM 生成摘要文本
6. 替换 session.messages:
   [{ role: User, content: "[Session History Summary]\n{摘要}" },
    ...提取的 skill 消息, ...提取的密语消息, ...保留的最后 N 轮]
7. 持久化: 将 compact 前的完整历史备份到 .krew/sessions/{id}.pre-compact.{unix_timestamp}.toml
8. 更新当前会话文件，显示 token 缩减统计和备份路径
```

**关键规则：**

- **备份可回滚**：compact 前自动备份，用户可手动恢复
- **摘要格式**：摘要作为 **user-role** 消息注入（前缀 `[Session History Summary]`），所有 Agent 共享
- **密语保留**：密语消息从压缩区提取并保留，不送入 LLM 压缩，完整保留其 `whisper_targets` 元数据
- **消息顺序**：`[Summary] + [skill messages] + [whisper messages] + [kept rounds]`

#### 3.5.3 自动压缩（Auto Compact）

当会话的最近一次 `prompt_tokens`（即实际发送给 LLM 的上下文大小）超过 `settings.auto_compact_threshold` 时，在下一次 Agent Loop 开始前自动触发压缩，无需用户手动执行 `/compact`。

**配置：**

```toml
[settings]
auto_compact_threshold = 120000   # 默认 120K tokens
```

**Token 计数来源：** 使用 LLM API 响应中返回的真实 `Usage` 数据（`prompt_tokens` / `completion_tokens`），而非字符估算。每次 Agent 回复完成时，`StreamEvent::Done(Usage)` 携带本次请求的精确用量，追加到 `Session.total_tokens_used` 并记录到 `ChatMessage.usage`。判断是否需要压缩时，取最近一次 LLM 请求的 `prompt_tokens` 值与阈值比较。

**各 Provider 的 Usage 返回方式：**

| Provider | 返回位置 | 字段 |
| -------- | -------- | ---- |
| OpenAI Chat | 流式最后一个 chunk 的 `usage` 字段（需设 `stream_options.include_usage = true`） | `prompt_tokens`, `completion_tokens`, `total_tokens` |
| OpenAI Responses | `response.completed` 事件的 `usage` 字段 | `input_tokens`, `output_tokens`, `total_tokens` |
| Anthropic | `message_delta` 事件的 `usage` 字段 + `message_start` 的 `usage` | `input_tokens`, `output_tokens`（无 `total_tokens`，由 Client 计算） |
| Google Gemini | 流式最后一个 chunk 的 `usageMetadata` 字段 | `promptTokenCount`, `candidatesTokenCount`, `totalTokenCount` |

**触发流程：**

```txt
Agent 回复完成，收到 StreamEvent::Done(usage)
    │
    ▼
更新 session.total_tokens_used += usage.total_tokens
记录 message.usage = usage
    │
    ▼
检查 usage.prompt_tokens >= auto_compact_threshold ?
    │
    ├── 否 ──→ 继续（等待下一次用户输入）
    │
    └── 是 ──→ 标记需要压缩
                │
                ▼
          下一次用户发送消息时，在 Agent Loop 开始前自动执行 compact
                │
                ▼
          使用 reply_order 中第一个 Agent 生成摘要
                │
                ▼
          显示提示: "⚡ 会话已自动压缩 (N tokens → M tokens)"
                │
                ▼
          继续 Agent Loop
```

**关键规则：**

- 使用 `reply_order` 中第一个 Agent 执行压缩（用户也可通过 `/compact <agent>` 手动选择）
- 自动压缩同样执行备份流程，确保可回滚
- `auto_compact_threshold = 0` 时禁用自动压缩
- 压缩后重置标记，下一次 Agent 回复后重新评估
- 密语消息从压缩区提取并保留（不发给 LLM 压缩，而是作为独立消息保留）

#### 3.5.4 /rewind 实现

`/rewind` 命令弹出 RewindPicker 选择截断点，使用 fork 语义：

```txt
1. 用户输入 /rewind
    │
    ▼
2. 弹出 RewindPicker popup（列出所有用户消息，时间正序，默认选中最后一条）
    │
    ▼
3. 用户选择消息（存储该消息在 self.messages 中的原始下标）
    │
    ├── 选中第一条用户消息 → 等同 /clear：保存当前完整 session，清屏，新 session ID
    │
    └── 选中非第一条消息 → fork:
         ├── 截断 self.messages 到选择点
         ├── 设置 rewound 标记（不立即保存）
         ├── 清屏并重放保留的消息
         └── 重建状态（token usage、last_respondent、skills）
    │
    ▼
4. 后续行为:
    ├── 发送新消息 → 生成新 session ID → 正常保存
    ├── /exit → 不保存截断后的 session
    ├── /clear → 不保存，清除 rewound 标记
    ├── /resume → 不保存，加载后清除 rewound 标记
    └── /compact → 拒绝执行，提示先发送新消息
```

#### 3.5.5 自定义命令

用户可通过 Markdown 文件定义自定义 Slash 命令。

**发现逻辑：**

使用 `discovery_paths()` 函数生成 6 层搜索路径（`.krew/commands/`、`.agents/commands/`、`.claude/commands/`，项目级 + 用户级），递归扫描子目录。子目录形成命名空间（如 `review/code.md` → `/review:code`）。同名命令 first-found wins，内置命令优先级高于自定义命令。

**Frontmatter 解析：**

```rust
struct CommandFrontmatter {
    description: Option<String>,
    argument_hint: Option<String>,  // YAML key: "argument-hint"
}
```

**参数替换：** `$ARGUMENTS` → 完整参数字符串；`$1`、`$2`... → 位置参数。

**Bash 预处理：** 参数替换后，扫描命令内容中的 `` !`command` `` 块，在 session cwd 中执行 Shell 命令，将块内容替换为 stdout 输出。执行失败时替换为错误消息（不中止）。最终内容通过 `parse_input()` 路由给 Agent。

#### 3.5.6 Skill 系统实现

**Skill 发现（`krew-core::skill_discovery`）：**

```rust
struct SkillRecord {
    name: String,
    description: String,
    location: PathBuf,       // SKILL.md absolute path
    base_dir: PathBuf,       // skill directory absolute path
    compatibility: Option<String>,
    metadata: Option<HashMap<String, String>>,
}

enum SkillError {
    ParseError(String),      // frontmatter parse failure
    MissingField(String),    // required field missing
    IoError(std::io::Error),
}

/// Discover skills from 7 priority-ordered paths.
/// Scan depth: 4 levels. Skip: .git/, node_modules/, target/.
fn discover_skills(cwd: &Path, home: &Path, extra_paths: &[String]) -> Vec<SkillRecord>;
```

**Skill Catalog 注入（`krew-core::skill_catalog`）：**

```rust
/// Build XML catalog from discovered skills.
fn build_skill_catalog(skills: &[SkillRecord]) -> Option<String>;
```

生成 `<available-skills>` XML 格式，注入到 system prompt 中（项目指令之后、Agent identity 之前），附带行为指令告知 LLM 何时调用 `activate_skill`。

**Skill 激活（`krew-tools::builtin::activate_skill`）：**

`activate_skill` 工具（只读，`requires_approval() = false`）接受 `name: String` 参数，返回 XML 包裹的 SKILL.md 正文内容 + Skill 目录绝对路径 + 资源文件列表（枚举 `scripts/`、`references/`、`assets/` 子目录，深度限制 2 层）。支持激活去重。

**Skill 配置（`krew-config`）：**

```rust
struct SkillsConfig {
    enabled: bool,                // default true
    extra_paths: Vec<String>,
}
```

### 3.6 Sub-Agent 系统

Sub-Agent 提供上下文隔离的子代理执行机制，让 Agent 将专项任务委派给专注的 Sub-Agent 在独立上下文中执行。

#### 3.6.1 发现机制

复用 `discovery::discovery_paths(cwd, "agents")` 生成扫描路径，在每个目录下扫描顶层 `*.md` 文件（非递归）。解析 YAML frontmatter（`name` + `description` 必需，`color`/`maxTurns` 可选），body 作为 system_prompt。Claude Code 兼容字段（`tools`、`model` 等）通过 `#[serde(flatten)]` 捕获并忽略。first-found-wins 去重。

代码位置：`krew-core::sub_agent::discovery`

#### 3.6.2 RunAgentTool 实现

`RunAgentTool` 在 `krew-core` 中实现 `krew_tools::ToolHandler` trait，注册到 `ToolRegistry` 中（与 MCP tool 注册方式一致）。

**核心字段：**
- `defs: HashMap<String, SubAgentDef>` — Sub-Agent 定义
- `is_running: Arc<AtomicBool>` — depth guard 防嵌套
- 父 Agent 运行时资源（`client`、`approval_cache` 等，构造时存储）
- `ToolRegistry` 不在构造时存储，而是执行时从 `ToolContext.tool_registry` 获取（避免 `Arc::get_mut` 注册失败）

**执行流程：**
1. CAS `is_running`（嵌套则报错 `"Sub-agent nesting is not allowed"`）
2. 从 `ctx.tool_registry` downcast 取得 `Arc<ToolRegistry>`
3. 从 `ctx.parent_event_tx` downcast 取得父 Agent event sender
4. 构建隔离 messages `[system_prompt, user(task)]`
5. 构建 `tool_defs` 时过滤掉 `run_agent`（LLM 看不到）
6. 调用 `run_agent_loop` 启动 Sub-Agent 循环
7. 通过 spawned consumer task 消费 sub_rx 事件：
   - `ToolCallStart/Output/Done` → 通过 `ctx.output_tx` 转发
   - `ApprovalRequest` → 通过 parent sender 转发到 TUI
   - `TextDelta` → 累积到 final_text（不转发）
8. Done 时返回 final_text 作为 tool result

代码位置：`krew-core::sub_agent::run_agent_tool`

#### 3.6.3 事件转发

```txt
Sub-Agent agent_loop
  │ (sub_tx) → AgentEvent
  ▼
RunAgentTool::execute() 消费 sub_rx
  │
  ├── ToolCallStart/Output/Done → ctx.output_tx (ToolCallOutput 管线)
  ├── ApprovalRequest           → parent_event_tx (downcast, 转发到父 channel)
  ├── TextDelta                 → 累积到 final_text (不转发)
  └── Done/Error                → return ToolResult
```

`parent_event_tx` 和 `tool_registry` 通过 `ToolContext` 的 `Option<Box<dyn Any + Send + Sync>>` 字段传递，由 `create_tool_context()` 在处理 `run_agent` tool 时分别设置为父 Agent 当前 turn 的 `UnboundedSender<AgentEvent>` clone 和 `Arc<ToolRegistry>` clone。

#### 3.6.4 防嵌套双重保障

1. **tool_defs 过滤**: Sub-Agent 的 LLM 看不到 `run_agent` tool
2. **execute() depth guard**: `Arc<AtomicBool>` CAS 硬保证，即使 prompt injection 触发也会拒绝

### 3.7 会话持久化

#### 3.7.1 TOML 文件存储

每个会话存储为一个 TOML 文件，文件名为会话 ID：

```txt
.krew/sessions/a1b2c3d4.toml
```

会话文件结构：

```toml
[session]
id = "a1b2c3d4"
cwd = "/path/to/project"
agents = ["gpt", "opus"]
total_tokens_used = 8926
created_at = "2026-02-28T14:30:00Z"
updated_at = "2026-02-28T15:45:00Z"

[[messages]]
role = "user"
addressee = "all"
content = "用 Rust 实现一个高性能的消息队列"
created_at = "2026-02-28T14:30:01Z"

[[messages]]
role = "assistant"
agent_name = "gpt"
content = "我建议使用 VecDeque 作为基础..."
created_at = "2026-02-28T14:30:15Z"

[messages.usage]
prompt_tokens = 1250
completion_tokens = 2034
total_tokens = 3284

[[messages]]
role = "assistant"
agent_name = "opus"
content = "考虑到高性能场景，推荐使用无锁环形缓冲区..."
created_at = "2026-02-28T14:30:32Z"

[messages.usage]
prompt_tokens = 3512
completion_tokens = 2130
total_tokens = 5642
```

#### 3.6.2 存储路径

```txt
.krew/                      (项目目录下)
├── settings.toml           -- 配置文件
├── sessions/               -- 会话目录
│   ├── a1b2c3d4.toml       -- 会话文件
│   └── e5f6g7h8.toml
└── logs/                   -- 日志目录
```

所有数据都存储在项目目录的 `.krew/` 下，跨平台统一。

#### 3.6.3 会话生命周期与 Rewound 状态

```txt
启动
  │
  ├── 无 --resume → 创建新 session（生成 ID，记录 cwd/agents/时间）
  │
  └── --resume → 加载历史 session（恢复消息历史到各 Agent 上下文）
  │
  ▼
运行中
  │
  ├── 用户消息 / Agent 回复 → 实时保存（原子写入: .tmp → rename）
  ├── /rewind → 设置 rewound 标记，延迟生成新 session ID
  ├── /compact → 备份后压缩
  ├── /clear → 保存当前 session，新建 session
  └── /resume → 保存当前 session，加载另一个 session
  │
  ▼
退出
  └── rewound 状态下 /exit → 不保存截断后的 session
```

**Rewound 状态守卫**：`save_session()` 统一检查 `rewound` 标记，标记为 true 时拒绝保存（避免截断后的历史覆盖原始完整历史）。发送新消息时先生成新 session ID，清除 `rewound` 标记，然后正常保存。

### 3.8 配置管理

#### 3.8.1 配置加载优先级

```txt
内置默认值
    ↓ 被覆盖
~/.krew/settings.toml         (用户级配置：providers、偏好、全局 MCP)
    ↓ 被覆盖
.krew/settings.toml           (项目级配置：agents、reply_order、项目覆盖)
    ↓ 被覆盖
CLI 参数                      (命令行覆盖)
```

加载流程：`UserConfig::load()` → `RawConfig::load()` → `merge_user()` → `resolve()` → `apply_cli_overrides()` → `validate()`。

`RawConfig` / `UserConfig` 为 partial 类型（settings 字段全部 `Option`），保留字段存在性用于合并。合并后由 `resolve()` 填充默认值生成最终 `Config`。

**合并规则：**

- 同名 provider 整项替换（project 覆盖 user）
- 同名 MCP server 替换，不同名追加
- 标量字段 project 优先
- `agents` 和 `reply_order` 仅在项目级配置中定义
- `--config` 指定路径时仍合并 user config

**配置校验（`Config::validate()`）：**

- `reply_order` 中引用的 agent 必须存在于 `agents` 列表
- agent 的 `provider` 必须存在于 `providers` 表（`"builtin"` 除外）
- agent `name` 不可重复
- `"all"` 为保留字，不可用作 agent 名称（大小写敏感，仅匹配全小写）

#### 3.8.2 配置数据结构

```rust
#[derive(Deserialize)]
struct Config {
    settings: Settings,
    agents: Vec<AgentConfig>,
    providers: HashMap<String, ProviderConfig>,
    mcp_servers: Vec<McpServerConfig>,
    skills: SkillsConfig,           // Skill 系统配置（默认 enabled=true）
    allow_rules: Vec<PermissionRule>,  // 顶层：自动放行的权限规则
    deny_rules: Vec<PermissionRule>,   // 顶层：自动拒绝的权限规则
    ask_rules: Vec<PermissionRule>,    // 顶层：强制确认的权限规则（即使 full-auto）
}

#[derive(Deserialize)]
struct Settings {
    approval_mode: ApprovalMode,    // suggest | auto-edit | full-auto
    reply_order: Vec<String>,       // @all 回答顺序
    auto_compact_threshold: Option<u32>,  // 会话自动压缩 token 阈值（默认 120000）
    compact_keep_rounds: Option<usize>,   // 压缩时保留最近 N 轮对话（默认 10）
    input_history_limit: Option<usize>,   // 输入历史上限（默认 1000）
    paste_burst_detection: Option<bool>,  // 粘贴检测（Windows 回退方案）
    worker_threads: Option<usize>,        // tokio 工作线程数（默认 4）
    other_agent_role: Option<OtherAgentRole>,  // 其他 Agent 消息的 role（默认 User）
    retry: Option<RetryConfig>,           // LLM 请求重试配置
    agent_to_agent_routing: Option<AgentToAgentRouting>,  // AI-to-AI 路由策略（默认 Immediate）
    agent_to_agent_max_rounds: Option<u32>,               // AI-to-AI 最大轮次（默认 10，0 = 禁用）
    restrict_workspace: Option<bool>,                     // 限制内建文件工具只能访问工作区目录（默认 true）
}

enum AgentToAgentRouting {
    Immediate,  // 目标 Agent 移到/插入队列头部（默认）
    Queued,     // 目标 Agent 追加到队列尾部
}

#[derive(Deserialize)]
struct AgentConfig {
    name: String,               // @ 寻址名
    display_name: String,
    provider: String,           // 引用 providers 表
    model: String,
    api_type: Option<ApiType>,  // OpenAI 系: "responses" | "chat"
    color: String,
    system_prompt: Option<String>,
    tools: bool,
    enable_web_search: bool,    // 启用模型原生 Web 搜索
    enable_thinking: bool,      // 启用思考/推理输出
    thinking_effort: Option<ThinkingEffort>,  // 思考力度: low | medium | high | max
    sampling: Option<SamplingConfig>,  // 采样参数（均可选，未设置则使用模型默认值）
}

enum ThinkingEffort {
    Low,
    Medium,
    High,
    Max,
}

/// Agent 级采样参数配置。
/// 所有字段均为 Option，未设置时使用各 Provider 的默认值。
/// 不同 Provider 支持的参数集不同，不支持的参数会被静默忽略。
#[derive(Deserialize)]
struct SamplingConfig {
    temperature: Option<f64>,           // 采样温度。OpenAI/Google: 0-2, Anthropic: 0-1
    top_p: Option<f64>,                 // 核采样。0-1。建议不与 temperature 同时调整
    top_k: Option<u32>,                 // Top-K 采样。仅 Anthropic/Google 支持
    max_tokens: Option<u32>,            // 最大输出 token 数。默认取模型最大值
    frequency_penalty: Option<f64>,     // 频率惩罚 -2.0~2.0。仅 OpenAI(Chat)/Google 支持
    presence_penalty: Option<f64>,      // 存在惩罚 -2.0~2.0。仅 OpenAI(Chat)/Google 支持
    stop_sequences: Option<Vec<String>>,// 停止序列
}

enum ApiType {
    Responses,  // OpenAI Responses API
    Chat,       // OpenAI Chat Completions API
}

#[derive(Deserialize)]
struct ProviderConfig {
    provider_type: ProviderType,        // openai | anthropic | google | vertex-anthropic
    api_key: Option<String>,            // 方式二：直接填写（不推荐）
    api_key_env: Option<String>,        // 方式一：环境变量名（优先）
    base_url: Option<String>,           // 自定义 endpoint；vertex-anthropic 可指向 passthrough root
    vertex_project: Option<String>,     // Google Vertex AI 项目 ID；vertex-anthropic 必填
    vertex_location: Option<String>,    // Google Vertex AI 区域（如 "global"、"us-east5"）
}

enum ProviderType {
    OpenAI,
    Anthropic,
    Google,
    VertexAnthropic,                    // serde rename = "vertex-anthropic"
}

#[derive(Deserialize)]
struct McpServerConfig {
    name: String,
    // Stdio 传输
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    // HTTP 传输
    url: Option<String>,                        // Streamable HTTP URL
    headers: Option<HashMap<String, String>>,    // HTTP 请求头
    // 通用
    trust: Option<McpTrust>,    // auto | confirm (默认 confirm)
}

enum McpTrust {
    Auto,       // 跳过审批
    Confirm,    // 按 approval_mode 确认（默认）
}

#[derive(Deserialize)]
struct SkillsConfig {
    enabled: bool,                  // 是否启用 Skill 系统（默认 true）
    extra_paths: Vec<String>,       // 额外的 Skill 搜索路径
}
```

#### 3.8.3 项目级指令文件（AGENTS.md）

krew 启动时自动扫描工作目录及其父目录链中的 `AGENTS.md` 文件，将内容注入到所有 Agent 的系统提示词中。

**常量定义（`krew-config`）：**

```rust
/// Filename to look for when loading project-level instructions.
pub const PROJECT_INSTRUCTIONS_FILENAME: &str = "AGENTS.md";

/// Maximum size in bytes for a single project instructions file (100KB).
pub const PROJECT_INSTRUCTIONS_MAX_SIZE: usize = 102_400;
```

**加载函数（`krew-config::instructions`）：**

```rust
/// Load project instructions by walking from `cwd` up to the filesystem root.
/// Files are merged ancestor-first (root → cwd) with a blank line separator.
/// Returns `None` if no instruction files are found.
pub fn load_project_instructions(cwd: &Path) -> Result<Option<String>, std::io::Error>;
```

**遍历策略：**

1. 从 `cwd` 开始，收集从 cwd 到文件系统根的所有目录路径
2. 反转列表得到祖先在前、cwd 在后的顺序
3. 依次检查每个目录下是否存在 `AGENTS.md`
4. 读取文件内容，跳过非 UTF-8 文件（记录 warning 日志）
5. 超过 100KB 的文件截断并追加 `[WARNING: File truncated at 100KB limit]`
6. 所有找到的内容以空行分隔合并

**注入格式（`krew-core::agent::build_system_prompt`）：**

System Prompt 层级合并顺序：

```txt
1. <project-instructions>        ← AGENTS.md 合并内容
   {AGENTS.md 合并内容}
   </project-instructions>

2. <available-skills>             ← Skill Catalog（如有 skills）
   <skill name="..." location="...">...</skill>
   </available-skills>
   {行为指令}

3. {agent system_prompt}          ← Agent 自身配置的 system_prompt
```

当无项目指令时跳过第 1 层；无可用 Skills 时跳过第 2 层。

**加载时机：** `App::new()` 中一次性加载，结果存储在 `App.project_instructions` 字段，所有 Agent 共享。文件不存在时静默返回 `None`。

#### 3.8.4 格式保留配置写入（Config Writer）

`krew-config::writer` 模块使用 `toml_edit` crate 实现格式保留的 TOML 配置文件写入，供 `krew config` 子命令使用。

```rust
/// Provider data for writing to config file.
struct ProviderWriteData {
    name: String,
    provider_type: ProviderType,        // openai | anthropic | google | vertex-anthropic
    api_key: Option<String>,
    api_key_env: Option<String>,
    base_url: Option<String>,
    vertex_project: Option<String>,
    vertex_location: Option<String>,
    extra_headers: Option<HashMap<String, String>>,
}

/// Agent data for writing to config file.
struct AgentWriteData {
    name: String,
    display_name: String,
    provider: String,
    model: String,
    color: String,
    enable_thinking: bool,
    enable_web_search: bool,
    tools: bool,                        // config wizard 固定为 true
    api_type: Option<String>,           // OpenAI 系: "responses" | "chat"
    system_prompt: Option<String>,
}
```

**核心函数：**

| 函数 | 功能 |
| ---- | ---- |
| `add_provider(path, data)` | 追加 provider 到 `[providers]` 表 |
| `remove_provider(path, name)` | 移除 `[providers.name]` |
| `add_agent(path, data)` | 追加 `[[agents]]` 条目，同步更新 `reply_order` |
| `remove_agent(path, name)` | 移除 `[[agents]]` 条目，同步更新 `reply_order` |
| `batch_add_agents(path, agents)` | 批量添加 agents（用于 init 的 Smart Preset） |
| `list_providers(path)` | 返回 `Vec<(String, ProviderConfig)>` |
| `list_agents(path)` | 返回 `(Vec<AgentConfig>, Vec<String>)` |

**关键特性：**
- 使用 `toml_edit::DocumentMut` 保留注释、空行和格式
- 写入前验证无重复条目
- 添加/删除 Agent 时自动维护 `settings.reply_order`
- 文件不存在时自动创建（含父目录）

#### 3.8.5 List Models API

`krew-llm::list_models` 模块提供从 LLM Provider API 获取可用模型列表的功能，供 config wizard 的 Smart Preset 模式使用。

```rust
struct ModelInfo {
    id: String,
}

struct ListModelsConfig {
    provider_type: ProviderType,
    base_url: Option<String>,
    api_key: String,
    vertex_project: Option<String>,
    vertex_location: Option<String>,
}

/// Fetch available models from provider API.
async fn list_models(config: &ListModelsConfig) -> Result<Vec<ModelInfo>, LlmError>;

/// Return hardcoded fallback model list for a provider type.
fn fallback_models(provider_type: ProviderType) -> Vec<ModelInfo>;
```

**各 Provider API 端点：**

| Provider | 端点 | 认证 | 过滤规则 |
| -------- | ---- | ---- | -------- |
| OpenAI（官方） | `GET {base_url}/v1/models` | Bearer token | `gpt`/`o`/`chatgpt` 前缀 |
| OpenAI（兼容，自定义 base_url） | `GET {base_url}/v1/models` | Bearer token | 不过滤，返回所有模型 |
| Anthropic | `GET {base_url}/v1/models` | `x-api-key` + `anthropic-version` | `claude-` 前缀 |
| Google Gemini | `GET generativelanguage.googleapis.com/v1beta/models?key=` | Query parameter | `gemini-` 前缀 |
| Google Vertex | `GET {location}-aiplatform.googleapis.com/v1/...` | Bearer token | `gemini-` 前缀 |
| Vertex Anthropic | `GET https://{host}/v1/projects/{project}/locations/{location}/publishers/anthropic/models` | Bearer token | `claude-` 前缀 |

Vertex Anthropic 的 `host` 规则与 chat/inference 一致：`global` 使用 `aiplatform.googleapis.com`，`us` 使用 `aiplatform.us.rep.googleapis.com`，`eu` 使用 `aiplatform.eu.rep.googleapis.com`，其他 region 使用 `{location}-aiplatform.googleapis.com`。当 `base_url` 指向 LiteLLM Vertex passthrough 或自定义代理时，URL builder 先去除尾部 `/`；若 base 以 `/v1` 结尾则直接追加 `/projects/...`，否则追加 `/v1/projects/...`。返回的 model name 去除 `publishers/anthropic/models/` 前缀，保留 Vertex 原生 ID。

超时：官方 OpenAI/Anthropic/Google 为 5 秒，OpenAI 兼容 Provider 为 15 秒（兼容服务可能返回大量模型）。结果按 ID 字母排序。API 调用失败时回退到硬编码列表。

#### 3.8.6 Config CLI 子命令

`krew-cli::config_cmd` 模块实现 `krew config` 子命令系统。所有子命令使用 `tokio::runtime::Builder::new_current_thread()` 轻量 runtime，不初始化 TUI terminal。

```rust
/// Dispatch config subcommands.
async fn dispatch(action: ConfigAction) -> i32;

enum ConfigAction {
    Init { user: bool, project: bool },
    Add { target: AddTarget },
    Del { target: DelTarget },
    List { target: ListTarget },
    Doctor,
    Help,
}
```

**模块结构：**

| 模块 | 功能 |
| ---- | ---- |
| `init` | 交互式初始化（智能路由 + Provider/Agent 创建 + Smart Preset） |
| `add` | 添加 Provider/Agent（复用 init 的交互逻辑） |
| `del` | 删除 Provider/Agent（带引用检查和确认） |
| `list` | 表格显示 Provider/Agent |
| `doctor` | 配置诊断（手动 TOML 解析，不使用 UserConfig::load() 避免静默回退） |
| `help` | 打印硬编码的配置手册 |

**交互组件：** 使用 `dialoguer` crate 的 `Select`、`FuzzySelect`、`Input`、`Password`、`Confirm` 实现终端交互。

### 3.9 TUI 实现细节

#### 3.9.1 Inline TUI 框架

基于 ratatui + crossterm，不使用 alternate screen，而是使用 inline viewport（自定义 Terminal 实现）。消息通过 `insert_before` 插入到 viewport 上方，viewport 动态调整高度。支持 keyboard enhancement（静默降级）。

#### 3.9.2 流式渲染管线

以 ~60Hz 频率执行 commit tick，驱动 token 队列 drain 和行插入。使用 `AdaptiveChunkingPolicy::decide()` 决定每次 drain 的数量，平衡渲染流畅性与响应速度。

#### 3.9.3 Agent 状态指示器

Agent 生成回复期间，在 viewport 分隔线上方显示状态行：闪烁 spinner（`●`/`◦`，600ms 间隔）、Agent 显示名、"Working" 文字、已用时间（紧凑格式：`45s` / `1m 23s` / `1h 05m`）、中断提示 "ESC to interrupt"。`ResponseStart` 出现，`Done`/`Error` 消失。

#### 3.9.4 Diff 渲染

`edit_file` 工具的审批 overlay 显示 colored diff 预览：

- **主题感知**：dark theme 使用深色背景，light theme 使用 GitHub pastel 背景
- **三级颜色深度**：TrueColor / ANSI-256 / ANSI-16 自动适配
- **行布局**：gutter（行号）+ sign（+/-/空格）+ content
- **语法高亮**：使用 syntect（超过 512KB 或 10,000 行跳过高亮）
- **Unicode 感知**：CJK 字符宽度计算（unicode-width）

#### 3.9.5 Pending Message 系统

Agent 响应期间用户可预排队消息，调度完成后自动提交。

**数据结构：**

```rust
const MAX_PENDING_MESSAGES: usize = 1;

struct PendingMessage {
    raw_input: String, // 不预解析，提交时重新 parse
}

// App state
pending_messages: VecDeque<PendingMessage>
```

**Enter 键三态逻辑：**

```txt
Enter (光标在最后一行):
  1. agent_event_rx.is_none() → send_message()
  2. agent_event_rx.is_some() + pending 未满 → queue_message()
  3. agent_event_rx.is_some() + pending 已满 → insert_newline()
```

**入队验证：** 非空 + 必须含 `@`/`#` 寻址（`LastRespondent` 被拒绝，textarea 保留）。原因：pending 期间 `last_respondent` 随串行执行和 A2A 触发而漂移，目标不确定。

**↑ 键双模式：** 光标在第一行时，有 pending → `pop_back()` 到 textarea（直接替换）；无 pending → 调取输入历史。

**Auto-drain：** Done / Error / Cancel 路径统一——`pending_agents` 清空且 `agent_event_rx` 变为 None 后，调用 `drain_pending_message()` 提交队首消息。

**Viewport 渲染：** Pending 区域位于 viewport 最上方（textarea 之上），包含标题行（`┄ 待发送 (N) ┄`）和消息行（`⏳ ● @name ...`，单行截断 + `…`）。Approval overlay / completion popup 活跃时隐藏。高度动态计算：`pending_area_height` + 现有布局。

#### 3.9.6 补全弹窗

输入 `/`、`@`、`#` 触发对应的补全弹窗，替换状态栏区域并扩展 viewport 高度。支持键盘导航（上下箭头、Tab/Enter 确认、Esc 关闭），同一时间只能显示一个弹窗。Slash 命令补全同时包含内置命令和自定义命令。

### 3.10 日志系统

使用 `tracing-subscriber` + `tracing-appender` 将日志写入 `.krew/logs/` 目录。按天滚动（daily rolling），默认保留 7 天，过期自动删除。日志不输出到 stdout/stderr（不干扰 TUI）。`--verbose` 参数切换 DEBUG/INFO 级别。

### 3.11 进程统计

`ProcessStats::collect()` 跨平台查询当前进程的物理内存占用（RSS）和线程数量：

| 平台 | 内存查询 | 线程查询 |
| ---- | -------- | -------- |
| Linux | `/proc/self/status` (VmRSS) | `/proc/self/status` (Threads) |
| Windows | `GetProcessMemoryInfo` | `CreateToolhelp32Snapshot` |
| macOS | `mach_task_basic_info` | `proc_pidinfo` |

失败时返回 `None` 而非报错。提供 `format_memory()` 格式化显示（`512 B` / `15.00 MB`）。

### 3.12 Agent Memory

跨会话持久化记忆系统，代码位于 `krew-core::memory` 模块。

#### 3.12.1 存储结构

两层目录：

```txt
.krew/memory/                          ← Global 层（所有 Agent 共享）
├── MEMORY.md                          ← 全局索引
├── user_role.md
└── agents/                            ← Per-Agent 层
    └── {agent_name}/
        ├── MEMORY.md                  ← Agent 索引
        └── feedback_no_emoji.md
```

记忆类型归属：`user` / `project` / `reference` → Global，`feedback` → Per-Agent。

#### 3.12.2 System Prompt 注入

`build_system_prompt()` 组装顺序：

```txt
Project Instructions → Skill Catalog → Sub-Agent Catalog → 【Memory Prompt】 → Agent Prompt
```

`load_memory_prompt(agent_name, cwd, has_tools)` 在每次 `start_completion()` 调用时执行：

1. `create_dir_all` 确保目录存在（失败静默返回 `None`）
2. 当 `has_tools = true` 时注入 `MEMORY_PROMPT_TEMPLATE`（含 `{{agent_name}}` 变量替换）
3. 读取 Global `MEMORY.md` → `read_and_truncate()` → 添加 `## Global Memory` 标题
4. 读取 Per-Agent `MEMORY.md` → `read_and_truncate()` → 添加 `## Your Memory` 标题
5. 当 `has_tools = false` 时跳过模板，仅注入索引内容

`has_tools` 由 `config.tools && !tools.specs().is_empty()` 决定——配置启用且实际有工具可用。

#### 3.12.3 MEMORY.md 截断

`read_and_truncate(path, MAX_LINES=200, MAX_BYTES=25000)`:

- 先按行数截断（保留前 200 行）
- 再按字节截断（在不超过 25KB 的最后完整行处切断）
- 截断时附加 `⚠ MEMORY.md is N lines (limit: 200). Only part of it was loaded.`

#### 3.12.4 Approval Carve-out

`check_tool_approval()` 8 步审批流水线中的 memory 处理：

```txt
Step 0: deny_rules → 可以 deny memory 路径
Step 1: bypass immunity → memory 路径（.krew/memory/**）跳过 dangerous-path 检查
Step 2: ask_rules → 可以对 memory 路径要求确认
Memory carve-out: is_memory_path() → 返回 Auto
Step 3+: 正常流程（memory 路径不会到达这里）
```

`is_memory_path(normalized)` 判断规则：`lower == ".krew/memory" || lower.starts_with(".krew/memory/")`。

---

## 4. 数据模型

### 4.1 核心类型

```rust
/// 统一消息格式（所有 Provider 通用）
struct ChatMessage {
    role: ChatRole,
    name: Option<String>,           // assistant 消息: Agent name; tool 消息: tool name
    content: String,
    tool_calls: Option<Vec<ToolCallInfo>>,   // assistant 消息携带的工具调用
    tool_call_id: Option<String>,            // tool 消息: 对应的 tool_call id
    server_tool_uses: Vec<ServerToolUseInfo>, // 服务端工具调用（Web Search 等）
    addressee: Option<String>,      // user 消息的目标: "all" | agent_name
    whisper_targets: Option<Vec<String>>,  // 密语目标 Agent 列表（设置时仅组内可见）
    usage: Option<Usage>,           // assistant 消息携带本次请求的 token 用量
    created_at: DateTime<Utc>,
}

enum ChatRole {
    System,
    User,
    Assistant,
    Tool,           // 工具结果消息
}

struct ToolCallInfo {
    id: String,
    name: String,
    arguments: String,  // JSON string
}

struct ServerToolUseInfo {
    name: String,
    query: Option<String>,
}
```

### 4.2 会话状态

```rust
struct Session {
    id: String,                     // UUID
    cwd: PathBuf,
    agents: Vec<String>,            // 参与的 Agent name 列表
    messages: Vec<ChatMessage>,     // 完整消息历史
    total_tokens_used: u64,         // 会话累计 token 用量（所有 Agent 的 total_tokens 之和）
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}
```

### 4.3 Agent 运行时状态

```rust
struct AgentRuntime {
    config: AgentConfig,
    client: Box<dyn LlmClient>,
    tools: Vec<Box<dyn Tool>>,
    is_responding: bool,            // 是否正在生成回复
}
```

---

## 5. 关键流程

### 5.1 消息发送流程（单 Agent @name）

```txt
 User Input: "@opus 解释一下这段代码"
     │
     ▼
 1. parse_input() → Addressee::Single("opus"), "解释一下这段代码"
     │
     ▼
 2. 构建 ChatMessage { role: User, content: "解释一下这段代码", addressee: "opus" }
     │
     ▼
 3. session.messages.push(message)
     │
     ▼
 4. storage.save_message(session_id, message)  -- 持久化
     │
     ▼
 5. agent_runtime["opus"].complete(session.messages)  -- 发起 LLM 请求
     │
     ▼
 6. 流式接收 StreamEvent::TextDelta(token)
     │    └── renderer.print_streaming("opus", token)  -- 实时渲染
     │
     ├── 收到 StreamEvent::ToolCall { name, args }
     │    └── 检查审批 → 执行工具 → 将结果追加消息 → 重新请求 LLM
     │
     ▼
 7. StreamEvent::Done(usage)
     │
     ▼
 8. 构建 Agent 回复 ChatMessage { role: Assistant, name: "opus", usage, ... }
     │
     ▼
 9. session.messages.push(reply)
     │    session.total_tokens_used += usage.total_tokens
     │
     ▼
10. storage.save_message(session_id, reply)
```

### 5.2 @all 广播流程

```txt
 User Input: "@all 你们觉得 Rust 和 Go 哪个更适合写 CLI？"
     │
     ▼
 1. parse_input() → Addressee::All
     │
     ▼
 2. 构建 ChatMessage { addressee: "all" }, 追加到 session 并持久化
     │
     ▼
 3. 按 reply_order = ["gpt", "opus", "gemini"] 串行执行:
     │
     ├─ [gpt] 执行完整 Agent Loop:
     │    gpt.run_loop(session.messages) → 流式输出 → 工具调用(如有)
     │    gpt 回复追加到 session.messages 并持久化
     │
     ├─ [opus] 执行完整 Agent Loop:
     │    opus 上下文已包含 gpt 的回复
     │    opus.run_loop(session.messages) → 流式输出 → 工具调用(如有)
     │    opus 回复追加到 session.messages 并持久化
     │
     └─ [gemini] 执行完整 Agent Loop:
          gemini 上下文已包含 gpt + opus 的回复
          gemini.run_loop(session.messages) → 流式输出 → 工具调用(如有)
          gemini 回复追加到 session.messages 并持久化
     │
     ▼
 4. pending_agents 清空 → drain_pending_message()
     └─ 如果用户在响应期间排队了消息，自动提交并开始新一轮调度
```

### 5.3 工具调用流程

```txt
 Agent 流式响应中收到 ToolCall
     │
     ▼
 检查 approval_mode
     │
     ├── 需要审批:
     │    渲染: ⚡ shell("cargo test") — 允许? [y/n]
     │    等待用户输入
     │    ├── y → 继续执行
     │    └── n → 返回 "工具调用被用户拒绝" 给 Agent
     │
     └── 不需要审批: 直接执行
              │
              ▼
         查找已注册的 Tool（内置 or MCP）
              │
              ▼
         tool.execute(args)
              │
              ▼
         构建 Tool role 消息（含 tool_call_id）
              │
              ▼
         追加到消息列表
              │
              ▼
         再次请求 LLM（携带工具结果）
              │
              ▼
         继续 Agent Loop（可能触发更多工具调用）
```

### 5.4 非交互式 Prompt 模式流程（-p）

```txt
 CLI 参数: krew -p "@claude review" (可选 stdin 管道)
     │
     ▼
 1. 验证参数（-p 与 --resume 互斥，--format 校验）
     │
     ▼
 2. 加载配置 → normalize
     │
     ▼
 3. parse_input(raw_prompt) → 解析 @agent/@all/#agent 寻址（仅从 -p 参数，不含 stdin）
     │  └── LastRespondent → 报错退出 (exit code 2)
     │  └── #all → 报错退出 (exit code 2)
     │  └── #agent → 密语模式（标记 whisper_targets，Agent 间可见性过滤）
     │
     ▼
 4. 读取 stdin（若有管道输入）→ <stdin>...</stdin> 包裹 → 拼接到 prompt 前方
     │
     ▼
 5. 初始化 Agents → 强制 FullAuto 审批模式（deny/ask 规则和保护路径免疫仍生效，需确认的调用自动拒绝）
     │
     ▼
 6. 初始化 MCP（若有配置）
     │
     ▼
 7. 构建 dispatch queue（根据寻址 + reply_order）
     │
     ▼
 8. 串行执行每个 Agent:
     │
     ├─ [agent_name] 调用 start_completion(messages)
     │    │
     │    ├── TextDelta → text 格式: 立即打印并 flush; json 格式: 缓存
     │    ├── ToolCallStart → 输出 ⚡ tool(args) 或 JSON {type:"tool_start"}
     │    ├── ToolCallOutput → 输出工具实时文本或 JSON {type:"tool_output"}
     │    ├── ToolCallDone → 输出 ⎿ summary 或 JSON {type:"tool_done"}
     │    ├── ServerToolStart → 输出 🌐 tool 或 JSON {type:"server_tool_start"}
     │    ├── ServerToolDone → 输出 🌐 done 或 JSON {type:"server_tool_done"}
     │    ├── ApprovalRequest → 自动 Approved
     │    ├── ThinkingDelta → 静默丢弃
     │    ├── Retrying → 输出到 stderr
     │    ├── Done(usage) → json 格式: 输出完整 text JSON
     │    └── Error → 输出到 stderr, 标记 has_error
     │
     ├─ 追加 Agent 回复到 messages
     ├─ 保存 session（每个 Agent 完成后增量保存）
     │
     └─ AI-to-AI 路由: 检测回复中的 @mention → 加入 dispatch queue
          └── 受 agent_to_agent_max_rounds 限制
     │
     ▼
 9. 退出: exit code 0 (全部成功) / 1 (有错误) / 2 (参数错误)
```

---

## 6. 项目结构

```txt
krew-cli/
├── Cargo.toml               # Workspace root（含 [profile.release] 优化配置）
├── AGENTS.md                # 项目级 Agent 指令
├── CLAUDE.md                # Claude Code 项目指令
├── crates/
│   ├── krew-cli/             # 主 CLI 入口 + TUI
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs             # 入口 + clap 解析 + mimalloc 全局分配器
│   │       ├── prompt_mode/        # 非交互式 prompt 模式（-p）
│   │       │   ├── mod.rs          # Prompt 模式主逻辑（寻址、调度、输出）
│   │       │   └── tests.rs        # Prompt 模式单元测试
│   │       ├── completion.rs       # @ / 补全状态管理
│   │       ├── textarea.rs         # 多行文本输入组件
│   │       ├── app/                # App 状态机 + 事件循环
│   │       │   ├── mod.rs
│   │       │   ├── state.rs        # App 状态定义
│   │       │   ├── input.rs        # 键盘事件处理 + ESC 取消 + pending 撤销
│   │       │   ├── commands.rs     # Slash 命令处理
│   │       │   ├── message.rs      # 消息发送 + pending 入队/drain
│   │       │   ├── agent_display.rs # Agent 显示信息管理
│   │       │   ├── persistence.rs  # 会话持久化集成
│   │       │   ├── paste_burst.rs  # 粘贴检测
│   │       │   └── approval/       # 工具审批 UI
│   │       │       ├── mod.rs
│   │       │       └── overlay.rs  # 审批 overlay 渲染
│   │       ├── render/             # TUI 渲染
│   │       │   ├── mod.rs
│   │       │   ├── viewport.rs     # 视口管理 + 状态栏
│   │       │   ├── messages.rs     # 消息渲染
│   │       │   ├── markdown.rs     # Markdown 渲染
│   │       │   ├── highlight.rs    # 语法高亮
│   │       │   ├── diff_render.rs  # Diff 预览渲染
│   │       │   ├── header.rs       # 启动 banner
│   │       │   ├── popup.rs        # 补全弹窗渲染
│   │       │   ├── color.rs        # 颜色工具
│   │       │   └── terminal_palette.rs  # 终端调色板
│   │       ├── custom_terminal/    # 自定义终端（行内视口）
│   │       │   ├── mod.rs
│   │       │   ├── terminal.rs
│   │       │   ├── frame.rs
│   │       │   └── ansi.rs
│   │       ├── frame_scheduler/    # 帧率调度
│   │       │   ├── mod.rs
│   │       │   ├── scheduler.rs
│   │       │   └── rate_limiter.rs
│   │       └── streaming/          # 流式输出管线
│   │           ├── mod.rs
│   │           ├── markdown_stream.rs  # Markdown 流式渲染
│   │           ├── chunking.rs         # Token 分块
│   │           └── commit_tick.rs      # 提交节拍
│   │
│   ├── krew-core/            # 核心逻辑
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── router.rs           # @ / # 寻址解析与消息路由
│   │       ├── command.rs          # Slash 命令定义与执行（含自定义命令发现）
│   │       ├── compact.rs          # /compact 实现（含密语消息保留）
│   │       ├── event.rs            # AgentEvent 类型定义
│   │       ├── persistence.rs      # 会话持久化逻辑（含 rewound 状态守卫）
│   │       ├── process_stats.rs    # 进程统计（内存、线程，跨平台）
│   │       ├── skill_discovery.rs  # Skill 发现与 SKILL.md 解析
│   │       ├── skill_catalog.rs    # Skill Catalog XML 构建
│   │       └── agent/              # Agent 运行时 + Agent Loop
│   │           ├── mod.rs
│   │           ├── agent_loop.rs   # Agent Loop 主循环（含工具调用循环）
│   │           ├── approval.rs     # 工具审批逻辑（含会话级缓存）
│   │           ├── init.rs         # Agent 初始化
│   │           ├── prepare.rs      # 消息准备与转换（含密语可见性过滤）
│   │           └── prune.rs        # 消息裁剪
│   │
│   ├── krew-llm/             # LLM Client 抽象
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # LlmClient trait
│   │       ├── types.rs            # 统一类型定义（StreamEvent, Usage 等）
│   │       ├── common.rs           # Provider 通用工具函数
│   │       ├── openai_responses.rs # OpenAI Responses API
│   │       ├── openai_chat.rs      # OpenAI Chat Completions API（含 Compatible）
│   │       ├── anthropic.rs        # Anthropic 实现
│   │       └── google.rs           # Google Gemini 实现
│   │
│   ├── krew-tools/           # 工具系统
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # ToolHandler trait + ToolSpec + ToolRegistry
│   │       ├── builtin/            # 内置工具
│   │       │   ├── mod.rs
│   │       │   ├── read_file.rs
│   │       │   ├── write_file.rs
│   │       │   ├── edit_file.rs
│   │       │   ├── shell.rs
│   │       │   ├── shell_parse.rs  # Shell 参数解析
│   │       │   ├── glob.rs
│   │       │   ├── grep.rs
│   │       │   ├── fetch_url.rs    # URL 抓取（HTML→Markdown）
│   │       │   └── activate_skill.rs  # Skill 激活工具
│   │       └── mcp/                # MCP 客户端
│   │           ├── mod.rs
│   │           ├── client.rs       # MCP 通信层
│   │           ├── manager.rs      # 多服务器管理
│   │           └── handler.rs      # MCP→ToolHandler 适配
│   │
│   ├── krew-storage/         # 持久化
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── session_file.rs     # TOML 会话文件读写
│   │       └── history_file.rs     # 输入历史持久化
│   │
│   └── krew-config/          # 配置管理
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── defaults.rs         # 内置默认配置
│           └── instructions.rs     # AGENTS.md 加载
│
├── .github/
│   └── workflows/
│       └── release.yml       # GitHub Actions 五平台构建 + Release
│
├── npm/                      # npm 分发包
│   ├── krew/                 # 主包 @zhing2026/krew
│   │   ├── package.json
│   │   └── bin/krew          # JS shim
│   └── krew-{platform}/     # 5 个平台子包
│       └── package.json
│
├── scripts/
│   ├── prepare-npm.sh        # 下载 Release 二进制到 npm 目录
│   └── npm-publish.sh        # 发布 npm 包
│
└── docs/
    ├── PDD.md
    ├── TDD.md
    └── dev_plan.md           # 开发计划与 Phase 进度
```

### 6.1 Crate 职责划分

| Crate | 职责 | 依赖 |
| ----- | ---- | ---- |
| `krew-cli` | CLI 入口、TUI 渲染、非交互式 prompt 模式、用户交互、审批 overlay、流式管线 | krew-core, krew-config, krew-llm, krew-tools, krew-storage |
| `krew-core` | 会话管理、Agent Loop、消息路由、Slash 命令、Compact、Skill 发现/Catalog、自定义命令 | krew-llm, krew-tools, krew-storage, krew-config |
| `krew-llm` | LLM API 抽象、各 Provider 实现、通用重试逻辑 | reqwest, serde, eventsource-stream |
| `krew-tools` | ToolHandler trait、ToolRegistry、内置工具（含 fetch_url、activate_skill）、MCP 客户端 | tokio, serde, rmcp, htmd, reqwest |
| `krew-storage` | TOML 文件会话持久化、输入历史 | toml, serde |
| `krew-config` | TOML 配置加载、AGENTS.md 指令加载 | toml, serde |

### 6.2 依赖关系图

```txt
krew-cli
  ├── krew-core
  │     ├── krew-llm
  │     ├── krew-tools
  │     ├── krew-storage
  │     └── krew-config
  └── krew-config
```

---

## 7. 依赖项汇总

### 7.0 依赖管理原则

- **避免重复依赖**：通过 `[workspace.dependencies]` 统一管理共享 crate 版本，避免同一 crate 出现多个版本（dup crates）
- **使用最新版本**：每个 crate 使用最新稳定版本，开发时定期 `cargo update`
- **版本范围精确**：workspace 中指定具体 minor 版本而非宽泛范围，减少解析歧义

### 7.1 Cargo.toml (workspace)

完整的 workspace 依赖列表见项目根目录 `Cargo.toml` 的 `[workspace.dependencies]` 段。原则：`default-features = false` 最小化依赖，通过 workspace 统一管理避免 dup crates。

### 7.2 各 Crate 特定依赖

**krew-cli**: `clap` (derive), `ratatui` (crossterm), `crossterm` (event-stream, bracketed-paste), `syntect`, `two-face`, `pulldown-cmark`, `similar`, `diffy`, `unicode-width`, `unicode-segmentation`, `textwrap`, `static_vcruntime` (Windows), `mimalloc` (Linux musl)
**krew-llm**: `reqwest`, `eventsource-stream`, `futures`, `async-trait`, `http`
**krew-tools**: `tokio`, `globset`, `regex`, `walkdir`, `dunce`, `rmcp`, `htmd`, `reqwest`
**krew-storage**: `toml`, `serde`, `chrono`
**krew-config**: `toml`, `serde`

---

## 8. 测试策略

### 8.1 单元测试

| 模块 | 测试重点 |
| ---- | ------- |
| `router.rs` | @ / # 寻址解析的各种边界情况（含密语） |
| `message.rs` | 消息序列化/反序列化（含 whisper_targets） |
| `command.rs` | Slash 命令识别与匹配（含自定义命令发现） |
| `config` | 配置加载、合并、默认值、skills 配置 |
| `prompt_mode` | stdin 拼接、寻址校验、输出格式、工具参数预览 |
| `skill_discovery` | Skill 发现、SKILL.md 解析、优先级 |
| `openai_responses.rs` / `openai_chat.rs` / `anthropic.rs` / `google.rs` | 消息格式转换正确性 |

### 8.2 集成测试

| 场景 | 测试内容 |
| ---- | ------- |
| 会话生命周期 | 新建 → 发送消息 → 写入 TOML → 恢复 → 验证消息完整性 |
| 工具调用 | mock LLM 返回 tool_call → 执行工具 → 验证结果回传 |
| 多 Agent 广播 | @all → 验证串行执行 + 上下文累积 |
| MCP | 启动 mock MCP server → 工具发现 → 调用验证 |

### 8.3 手动测试

- 实际接入各 LLM API 进行端到端对话测试
- 不同终端模拟器的渲染兼容性测试
- 长会话的 token 管理与 `/compact` 功能验证

---

## 附录 A: 与 codex 架构对照

| 方面 | codex | krew-cli |
| ---- | ----- | -------- |
| Agent 数量 | 单 Agent | 多 Agent |
| 消息模型 | SQ/EQ 队列 | 简化的 `Vec<Message>` |
| LLM 接入 | WebSocket + HTTP SSE | HTTP SSE（更简单） |
| TUI | Ratatui 全功能 | Ratatui 简化版 |
| 会话存储 | SQLite + JSONL | TOML 文件 |
| 工具系统 | MCP + 内置 + 沙箱 | MCP + 内置（无沙箱） |
| 配置 | TOML 多层 | TOML 双层（用户级 + 项目级） |
| 审批系统 | 多级策略 | 简化三级策略 |

---

## 附录 B: 后续迭代方向

- ~~**v0.2**: Agent Skills 支持（为 Agent 配置特定技能/能力）~~ ✅ 已完成
- ~~**v0.3**: 自定义 Slash 命令（用户可扩展命令系统）~~ ✅ 已完成
- ~~**v0.4**: Agent 间可 @对方 形成 AI-to-AI 对话（支持 immediate/queued 两种路由策略，`agent_to_agent_max_rounds` 轮次限制）~~ ✅ 已完成
- ~~**v0.4.2**: 非交互式 Prompt 模式（`-p`），支持 stdin 管道、text/json 输出格式、全自动审批、AI-to-AI 路由、session 持久化~~ ✅ 已完成
- ~~**v0.5.0**: 密语模式（`#agent`），支持单目标/多目标私密消息、密语组内 A2A、消息可见性过滤、密语压缩保留、TUI/P 模式锁图标显示~~ ✅ 已完成
- ~~**v0.5.1**: `/rewind` 会话回退命令（fork 语义、RewindPicker UI、rewound 状态管理）~~ ✅ 已完成
- ~~**v0.5.2**: LiteLLM 代理支持（OpenAI Responses API 自定义 base_url）~~ ✅ 已完成
- ~~**v0.5.3**: Markdown 软换行处理、OpenAI Chat reasoning_effort 支持~~ ✅ 已完成
- ~~**v0.5.4**: OpenAI Chat Web Search 支持~~ ✅ 已完成
- ~~**v0.5.5**: `settings.language` 多语言响应指令、Gemini web_search 误报修复~~ ✅ 已完成
- ~~**v0.6.0**: 配置管理子命令系统（`krew config init/add/del/list/doctor/help`），交互式配置向导、格式保留 TOML 写入、List Models API、配置诊断~~ ✅ 已完成
