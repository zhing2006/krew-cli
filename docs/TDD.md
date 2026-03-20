# krew-cli — 技术设计文档 (TDD)

> 版本: 0.1.0 | 日期: 2026-03-06
> 参考: [PDD](./PDD.md) | 参考项目: [codex CLI](https://github.com/openai/codex)

---

## 1. 架构总览

### 1.1 系统分层

```txt
┌──────────────────────────────────────────────────┐
│                    CLI Layer                     │
│         (clap 命令解析 + TUI 交互渲染)             │
├──────────────────────────────────────────────────┤
│                  Session Layer                   │
│       (会话管理 / 消息路由 / @ 寻址解析)           │
├──────────────┬──────────────┬────────────────────┤
│  Agent Loop  │  Tool System │  Slash Commands    │
│  (Agent 循环) │ (工具调度)    │  (命令处理)        │
├──────────────┴──────────────┴────────────────────┤
│               LLM Client Layer                   │
│  (多 Provider 统一抽象: OpenAI/Anthropic/Google/  │
│   OpenAI-Compatible)                              │
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
| `toml` | TOML 配置/会话文件序列化 | 0.9 |
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
fn parse_input(input: &str, known_agents: &[String]) -> Result<(Addressee, String)> {
    let input = input.trim();
    if input.is_empty() {
        return Err(anyhow!("empty input"));
    }

    // Scan all whitespace-delimited words for @name tokens.
    let mut matched: Vec<String> = Vec::new();
    for word in input.split_whitespace() {
        if let Some(name) = word.strip_prefix('@') {
            if (name == "all" || known_agents.contains(&name.to_string()))
                && !matched.contains(&name.to_string())
            {
                matched.push(name.to_string());
            }
        }
    }

    let message = input.to_string(); // full original input, not stripped

    if matched.is_empty() {
        Ok((Addressee::LastRespondent, message))
    } else if matched.iter().any(|n| n == "all") {
        Ok((Addressee::All, message))  // @all takes priority
    } else if matched.len() == 1 {
        Ok((Addressee::Single(matched[0].clone()), message))
    } else {
        Ok((Addressee::Multiple(matched), message))
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

#### 3.2.2 路由规则

| 寻址 | 接收 LLM 请求的 Agent | 消息可见性 |
| ---- | -------------------- | --------- |
| `@all` | 所有 Agent（按 reply_order 串行） | 全部可见 |
| `@gpt` | 仅 gpt | 全部可见（上下文共享） |
| `@gpt @opus` | gpt 和 opus（按 @ 出现顺序串行） | 全部可见 |
| 无识别的 @ | 上一个回答者（无则提示指定） | 全部可见 |

### 3.3 LLM Client 抽象层

#### 3.3.1 统一 Trait

```rust
#[async_trait]
trait LlmClient: Send + Sync {
    /// 发起流式对话请求
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
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

##### OpenAI Client

同时支持两种 API，通过 Agent 配置的 `api_type` 字段选择：

- **Responses API** (`api_type = "responses"`)
  - API: `POST /v1/responses` (stream=true)
  - 请求格式: `{ model, input: [...], tools: [...], stream: true }`
  - 响应事件: `response.output_item.added`, `response.output_text.delta`, `response.completed` 等
  - Web Search: tools 中添加 `{ type: "web_search_preview" }`

- **Chat Completions API** (`api_type = "chat"`)
  - API: `POST /v1/chat/completions` (stream=true)
  - 请求格式: `{ model, messages: [...], tools: [...], stream: true }`
  - 响应事件: 标准 SSE `data: {"choices":[{"delta":...}]}`

##### Anthropic Client

- API: `POST /v1/messages` (stream=true)
- 使用 SSE 解析流式响应
- 支持 tool_use (tools 参数)
- 消息格式: `{ role, content: [{ type: "text" | "tool_use" | "tool_result" }] }`
- 需要处理 Anthropic 特有的 content block 结构
- Web Search: tools 中添加 `{ type: "web_search_20250305", name: "web_search" }`，响应含 `web_search_tool_result` block 和 `citations`

##### Google Client

- API: Gemini `generateContent` (stream=true)
- 使用 SSE 解析流式响应
- 支持 function calling
- 消息格式: `{ role, parts: [{ text } | { functionCall }] }`
- Web Search: tools 中添加 `{ google_search: {} }`，响应含 `groundingMetadata` 包括 `groundingChunks` 和 `groundingSupports`

##### OpenAI-Compatible Client

- 复用 OpenAI Client 实现，替换 base_url 和认证方式
- 用于接入豆包（ByteDance）等第三方 OpenAI 兼容服务
- 同样支持 `api_type` 配置（默认 `chat`）
- Web Search: 取决于具体服务是否支持

#### 3.3.3 消息格式转换

每个 Provider 实现中包含一个 `convert_messages` 方法，将统一的 `ChatMessage` 转换为各 Provider 特定的 API 请求格式。**转换时需要感知"当前是哪个 Agent"**，以正确设置 role。

**核心问题：其他 Agent 的回复用什么 role 发送？**

对于 Agent "opus" 视角下的消息历史：

| 原始消息 | 发送给 opus 时的 role | 处理方式 |
| -------- | -------------------- | -------- |
| 用户消息 | `user` | 直接发送 |
| opus 自己之前的回复 | `assistant` | 直接发送 |
| gpt 的回复 | **待定（需测试）** | 方案 A 或 B |

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
| OpenAI (Responses API) | `tools` 数组加 `{ type: "web_search_preview" }` | `web_search_preview` |
| Anthropic | `tools` 数组加 `{ type: "web_search_20250305", name: "web_search" }` | `web_search_20250305` |
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

### 3.3.7 安全边界

**路径边界**：内置文件工具（read_file, write_file, edit_file, glob, grep）在执行前校验路径：

- 解析后的绝对路径必须在 `session.cwd` 及其子目录内
- 拒绝包含 `..` 的路径穿越尝试
- 符号链接解析后仍需在边界内

**MCP 信任级别**：McpServerConfig 的 `trust` 字段（见 3.7.2 配置数据结构）控制该 MCP server 的审批行为。`trust = "auto"` 时跳过审批，`trust = "confirm"`（默认）时按 approval_mode 规则确认。

### 3.4 工具系统

#### 3.4.1 工具注册

```rust
trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;  // JSON Schema
    fn requires_approval(&self) -> bool;
    async fn execute(&self, args: serde_json::Value) -> Result<ToolResult>;
}

struct ToolResult {
    content: String,
    is_error: bool,
}
```

#### 3.4.2 内置工具列表

| 工具名 | 功能 | 类别 | suggest 下 | auto-edit 下 | full-auto 下 |
| ------ | ---- | ---- | --------- | ----------- | ----------- |
| `read_file` | 读取文件内容 | 读操作 | 自动 | 自动 | 自动 |
| `write_file` | 写入文件 | 写操作 | 需确认 | 自动 | 自动 |
| `edit_file` | 搜索替换编辑 | 写操作 | 需确认 | 自动 | 自动 |
| `shell` | 执行 Shell 命令 | Shell | 需确认 | 需确认 | 自动 |
| `glob` | 文件名模式匹配 | 读操作 | 自动 | 自动 | 自动 |
| `grep` | 文件内容搜索 | 读操作 | 自动 | 自动 | 自动 |
| `fetch_url` | 抓取 URL 内容（HTML→Markdown） | 网络 | 白名单域名自动，其他需确认 | 同左 | 自动 |

#### 3.4.3 MCP 集成

MCP 实现分为三个模块：

- `McpClient` — 底层通信，封装 rmcp SDK，支持 stdio 和 Streamable HTTP 两种传输
- `McpManager` — 管理多个 MCP 服务器的生命周期，提供统一的工具发现和调用接口
- `McpToolHandler` — 将 MCP 工具适配为内置 Tool trait，统一注册到工具系统

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

MCP 服务器在会话启动时初始化，发现的工具自动注册到工具系统中，与内置工具统一暴露给 Agent。工具的审批级别由 MCP 服务器的 `trust` 配置和工具的 `annotations` 元数据共同决定。

#### 3.4.4 工具审批流程

```txt
Agent 请求工具调用
    │
    ▼
检查审批策略(approval_mode) + 工具类型
    │
    │  读操作(read_file/glob/grep): 所有策略下均自动执行
    │
    ├── full-auto ──→ 所有工具直接执行
    │
    ├── auto-edit ──→ 写操作(write_file/edit_file)自动执行
    │                  Shell/MCP 需确认
    │
    └── suggest ───→ 写操作/Shell/MCP 均需确认
                          │
                          ▼
                    渲染审批提示
                    ⚡ shell("cargo test") — 允许? [y/n/always]
                          │
                          ▼
                    用户选择 ──→ 执行或跳过
```

### 3.5 Slash 命令系统

```rust
enum SlashCommand {
    Clear,              // 清屏（同 /new）
    Resume,             // 恢复历史会话
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

命令发现：输入 `/` 后，匹配所有命令名前缀，弹出补全列表。

#### 3.5.1 /agents 输出规格

`/agents` 命令输出当前会话的 Agent 列表及 token 用量统计。

**输出格式：**

```txt
Agents in session:
  [gpt]    GPT-5.2          openai/gpt-5.2           3,284 tokens (1,250 in / 2,034 out)
  [opus]   Claude Opus      anthropic/claude-opus-4-6 5,642 tokens (3,512 in / 2,130 out)
──────────────────────────────────────────────────────
  Total: 8,926 tokens
```

**聚合规则：**

- 遍历 `session.messages` 中所有 `role = Assistant` 的消息，按 `agent_name` 分组
- 每个 Agent 累加其所有消息的 `usage.prompt_tokens` 和 `usage.completion_tokens`
- Total 行使用 `session.total_tokens_used`

#### 3.5.2 /compact 实现方案

`/compact <agent_name>` 使用指定 Agent 将当前会话历史压缩为一段摘要。

**流程：**

```txt
1. 取 session.messages 中除最后 N 条外的所有消息作为"待压缩区"（N 可配，默认保留最后 3 轮）
2. 构建压缩请求: system="请将以下对话历史压缩为简洁摘要" + 待压缩区消息
3. 调用指定 Agent 的 LLM 生成摘要文本
4. 替换 session.messages:
   [{ role: System, content: "会话历史摘要:\n{摘要}" }, ...保留的最后 N 条]
5. 持久化: 将 compact 前的完整历史备份到 .krew/sessions/{id}.pre-compact.{timestamp}.toml
6. 更新当前会话文件
```

**关键规则：**

- **备份可回滚**：compact 前自动备份，用户可手动恢复
- **跨 Agent 一致性**：压缩后的摘要作为 System 消息注入所有 Agent 的上下文，保证所有 Agent 看到相同的历史摘要
- **持久化格式**：摘要存储为 session 文件中的 `[compact_summary]` 段

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

### 3.6 会话持久化

#### 3.6.1 TOML 文件存储

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

### 3.7 配置管理

#### 3.7.1 配置加载优先级

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

#### 3.7.2 配置数据结构

```rust
#[derive(Deserialize)]
struct Config {
    settings: Settings,
    agents: Vec<AgentConfig>,
    providers: HashMap<String, ProviderConfig>,
    mcp_servers: Vec<McpServerConfig>,
}

#[derive(Deserialize)]
struct Settings {
    approval_mode: ApprovalMode,    // suggest | auto-edit | full-auto
    reply_order: Vec<String>,       // @all 回答顺序
    auto_compact_threshold: Option<u32>,  // 会话自动压缩 token 阈值（默认 120000）
    compact_keep_rounds: Option<usize>,   // 压缩时保留最近 N 轮对话（默认 3）
    input_history_limit: Option<usize>,   // 输入历史上限（默认 1000）
    paste_burst_detection: Option<bool>,  // 粘贴检测（Windows 回退方案）
    worker_threads: Option<usize>,        // tokio 工作线程数（默认 4）
    other_agent_role: Option<OtherAgentRole>,  // 其他 Agent 消息的 role（默认 User）
    retry: Option<RetryConfig>,           // LLM 请求重试配置
    shell_allow_commands: Option<Vec<String>>,  // 免审批 shell 命令前缀列表
    fetch_allow_domains: Option<Vec<String>>,   // 免审批 fetch_url 域名白名单
    agent_to_agent_routing: Option<AgentToAgentRouting>,  // AI-to-AI 路由策略（默认 Immediate）
    agent_to_agent_max_rounds: Option<u32>,               // AI-to-AI 最大轮次（默认 10，0 = 禁用）
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
    thinking_effort: Option<ThinkingEffort>,  // 思考力度: low | medium | high
    sampling: Option<SamplingConfig>,  // 采样参数（均可选，未设置则使用模型默认值）
}

enum ThinkingEffort {
    Low,
    Medium,
    High,
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
    provider_type: ProviderType,        // openai | anthropic | google
    api_key: Option<String>,            // 方式二：直接填写（不推荐）
    api_key_env: Option<String>,        // 方式一：环境变量名（优先）
    base_url: Option<String>,
    vertex_project: Option<String>,     // Google Vertex AI 项目 ID
    vertex_location: Option<String>,    // Google Vertex AI 区域（如 "us-central1"）
}

enum ProviderType {
    OpenAI,
    Anthropic,
    Google,
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
```

#### 3.7.3 项目级指令文件（AGENTS.md）

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

```rust
/// Build the final system prompt by merging project instructions
/// with the agent's configured system_prompt.
pub fn build_system_prompt(
    project_instructions: Option<&str>,
    agent_system_prompt: Option<&str>,
) -> Option<String>;
```

输出格式：

```txt
<project-instructions>
{AGENTS.md 合并内容}
</project-instructions>

{agent system_prompt}
```

当无项目指令时，直接使用 `system_prompt` 原值。

**加载时机：** `App::new()` 中一次性加载，结果存储在 `App.project_instructions` 字段，所有 Agent 共享。文件不存在时静默返回 `None`。

---

## 4. 数据模型

### 4.1 核心类型

```rust
/// 统一消息格式（所有 Provider 通用）
struct ChatMessage {
    role: Role,
    agent_name: Option<String>,     // assistant 消息标明来源 Agent
    addressee: Option<String>,      // user 消息的目标: "all" | agent_name（恢复会话时用于还原对话流）
    content: MessageContent,
    tool_calls: Option<Vec<ToolCall>>,
    tool_results: Option<Vec<ToolCallResult>>,
    usage: Option<Usage>,           // assistant 消息携带本次请求的 token 用量
    created_at: DateTime<Utc>,
}

enum Role {
    System,
    User,
    Assistant,
    Tool,           // 工具结果消息
}

enum MessageContent {
    Text(String),
    Blocks(Vec<ContentBlock>),  // Anthropic 风格的多 block 内容
}

struct ToolCall {
    id: String,
    name: String,
    arguments: serde_json::Value,
}

struct ToolCallResult {
    tool_call_id: String,
    content: String,
    is_error: bool,
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
 8. 构建 Agent 回复 ChatMessage { role: Assistant, agent_name: "opus", usage, ... }
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
         构建 ToolCallResult
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
│   │       ├── completion.rs       # @ / 补全状态管理
│   │       ├── textarea.rs         # 多行文本输入组件
│   │       ├── app/                # App 状态机 + 事件循环
│   │       │   ├── mod.rs
│   │       │   ├── state.rs        # App 状态定义
│   │       │   ├── input.rs        # 键盘事件处理 + ESC 取消
│   │       │   ├── commands.rs     # Slash 命令处理
│   │       │   ├── message.rs      # 消息发送流程
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
│   │       ├── router.rs           # @ 寻址解析与消息路由
│   │       ├── command.rs          # Slash 命令定义与执行
│   │       ├── compact.rs          # /compact 实现
│   │       ├── event.rs            # 事件类型定义
│   │       ├── persistence.rs      # 会话持久化逻辑
│   │       ├── process_stats.rs    # 进程统计（内存、线程）
│   │       └── agent/              # Agent 运行时 + Agent Loop
│   │           ├── mod.rs
│   │           ├── agent_loop.rs   # Agent Loop 主循环
│   │           ├── approval.rs     # 工具审批逻辑
│   │           ├── init.rs         # Agent 初始化
│   │           ├── prepare.rs      # 消息准备与转换
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
│   │       ├── lib.rs              # Tool trait + 注册
│   │       ├── builtin/            # 内置工具
│   │       │   ├── mod.rs
│   │       │   ├── read_file.rs
│   │       │   ├── write_file.rs
│   │       │   ├── edit_file.rs
│   │       │   ├── shell.rs
│   │       │   ├── shell_parse.rs  # Shell 参数解析
│   │       │   ├── glob.rs
│   │       │   ├── grep.rs
│   │       │   └── fetch_url.rs    # URL 抓取（HTML→Markdown）
│   │       └── mcp/                # MCP 客户端
│   │           ├── mod.rs
│   │           ├── client.rs       # MCP 通信层
│   │           ├── manager.rs      # 多服务器管理
│   │           └── handler.rs      # MCP→Tool trait 适配
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
| `krew-cli` | CLI 入口、TUI 渲染、用户交互、审批 overlay、流式管线 | krew-core, krew-config, krew-llm, krew-tools, krew-storage |
| `krew-core` | 会话管理、Agent Loop、消息路由、Slash 命令、Compact | krew-llm, krew-tools, krew-storage, krew-config |
| `krew-llm` | LLM API 抽象、各 Provider 实现 | reqwest, serde, eventsource-stream |
| `krew-tools` | 工具 trait、内置工具（含 fetch_url）、MCP 客户端 | tokio, serde, rmcp, htmd, reqwest |
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
| `router.rs` | @ 寻址解析的各种边界情况 |
| `message.rs` | 消息序列化/反序列化 |
| `command.rs` | Slash 命令识别与匹配 |
| `config` | 配置加载、合并、默认值 |
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
