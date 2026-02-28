# krew-cli — 技术设计文档 (TDD)

> 版本: 0.1.0 | 日期: 2026-02-28
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

产出单文件可执行程序，零外部依赖，三平台均静态链接：

| 平台 | 策略 | 关键配置 |
| ---- | ---- | -------- |
| **Windows** | `static_vcruntime` crate 静态链接 MSVC 运行时 | 在 krew-cli 的 Cargo.toml 中依赖 `static_vcruntime` |
| **Linux** | 使用 musl target 生成全静态二进制 | `--target x86_64-unknown-linux-musl`；使用 `mimalloc` 替换 musl 默认分配器以避免性能问题 |
| **macOS** | 静态链接 CRT | `RUSTFLAGS="-C target-feature=+crt-static"`；系统框架仍动态链接（Apple 限制） |

**TLS 依赖**：reqwest 0.13 默认使用 rustls，无需额外配置。关闭 `default-features` 后通过 `rustls` feature 显式启用，避免 musl 下 OpenSSL 静态编译问题。

### 2.3 关键 Crate

所有 crate 使用最新稳定版，`default-features = false` 最小化依赖，通过 workspace 统一管理避免 dup crates。

| Crate | 用途 | 版本 |
| ----- | ---- | ---- |
| `clap` | CLI 参数解析 (derive) | 4 |
| `tokio` | 异步运行时 | 1 |
| `ratatui` | TUI 渲染（内含 crossterm 重新导出，无需单独依赖） | 0.30 |
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
| `static_vcruntime` | Windows MSVC 运行时静态链接（仅 Windows） | 2 |
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
    All,                     // @all
    Single(String),          // @gpt, @opus 等
    LastRespondent,          // 无前缀: 发给上一个回答者
}

/// 解析用户输入，返回寻址目标和消息正文。
/// 空输入或无效格式返回 Err。
fn parse_input(input: &str) -> Result<(Addressee, String)> {
    let input = input.trim();
    if input.is_empty() {
        return Err(anyhow!("输入不能为空"));
    }

    if input == "@all" || input.starts_with("@all ") {
        // "@all" 或 "@all 消息"（不匹配 @allxxx）
        let message = input.strip_prefix("@all").unwrap().trim_start().to_string();
        Ok((Addressee::All, message))
    } else if let Some(at_rest) = input.strip_prefix('@') {
        // "@name 消息" 或 "@name"
        let (name, message) = match at_rest.split_once(' ') {
            Some((n, m)) => (n.to_string(), m.to_string()),
            None => (at_rest.to_string(), String::new()), // @name 无正文: 正文为空
        };
        if name.is_empty() {
            return Err(anyhow!("@ 后需指定 Agent 名称"));
        }
        Ok((Addressee::Single(name), message))
    } else {
        Ok((Addressee::LastRespondent, input.to_string()))
    }
}
```

**边界情况处理：**
- `@all`（无正文）→ `Addressee::All` + 空消息，发送空消息给所有 Agent
- `@opus`（无正文）→ `Addressee::Single("opus")` + 空消息
- `@`（无名称）→ 报错
- 空输入 → 报错

#### 3.2.2 路由规则

| 寻址 | 接收 LLM 请求的 Agent | 消息可见性 |
| ---- | -------------------- | --------- |
| `@all` | 所有 Agent | 全部可见 |
| `@gpt` | 仅 gpt | 全部可见（上下文共享） |
| 无前缀 | 上一个回答者（无则提示指定） | 全部可见 |

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
    /// 流结束
    Done,
    /// 错误
    Error(String),
}
```

#### 3.3.2 Provider 实现

**OpenAI Client**

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

- **Azure 模式**: 当 `azure_endpoint` 有值时启用，替换 base_url，添加 `api-version` 查询参数和 `api-key` header。Azure 同样支持 Responses 和 Chat 两种 API，按 `api_type` 选择

**Anthropic Client**

- API: `POST /v1/messages` (stream=true)
- 使用 SSE 解析流式响应
- 支持 tool_use (tools 参数)
- 消息格式: `{ role, content: [{ type: "text" | "tool_use" | "tool_result" }] }`
- 需要处理 Anthropic 特有的 content block 结构
- Web Search: tools 中添加 `{ type: "web_search_20250305", name: "web_search" }`，响应含 `web_search_tool_result` block 和 `citations`

**Google Client**

- API: Gemini `generateContent` (stream=true)
- 使用 SSE 解析流式响应
- 支持 function calling
- 消息格式: `{ role, parts: [{ text } | { functionCall }] }`
- Web Search: tools 中添加 `{ google_search: {} }`，响应含 `groundingMetadata` 包括 `groundingChunks` 和 `groundingSupports`

**OpenAI-Compatible Client**

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

**方案 A：其他 Agent 的回复作为 `user` role**
```
{ role: "user", content: "[gpt] GPT-5.2:\n我建议使用 VecDeque..." }
```
优点：LLM 不会混淆自己和别人的发言。
缺点：LLM 可能把其他 Agent 的专业回复当作"用户说的"。

**方案 B：其他 Agent 的回复作为 `assistant` role**
```
{ role: "assistant", content: "[gpt] GPT-5.2:\n我建议使用 VecDeque..." }
```
优点：LLM 知道这是 AI 级别的回复。
缺点：LLM 可能以为是自己说的，产生混淆。

> **决策：此问题需要实际测试后确定。** `convert_messages` 方法接收 `self_agent_name` 参数，根据配置选择方案 A 或 B。初始实现两种方案都支持，通过配置切换。

```rust
// Responses API 和 Chat Completions API 各自实现 convert_messages，
// 但都接收相同的参数以区分 Agent 身份
fn convert_messages(
    messages: &[ChatMessage],
    self_agent_name: &str,          // 当前 Agent 的 name
    other_agent_role: OtherAgentRole, // User 或 Assistant
) -> Vec<ProviderMessage> { ... }

enum OtherAgentRole {
    User,       // 方案 A
    Assistant,  // 方案 B
}
```

### 3.3.4 错误处理与重试

LLM API 调用的错误处理策略：

| 错误类型 | 处理方式 |
| -------- | -------- |
| 429 Rate Limit | 指数退避重试（1s → 2s → 4s），最多 3 次 |
| 5xx 服务端错误 | 重试 2 次，间隔 2s |
| 网络超时 | 超时阈值 60s（流式首 token）/ 300s（流式总时长），超时后重试 1 次 |
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

### 3.3.6 安全边界

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

#### 3.4.3 MCP 集成

```rust
struct McpServer {
    name: String,
    process: Child,          // 子进程
    transport: StdioTransport,  // stdin/stdout JSON-RPC
}

impl McpServer {
    async fn initialize(&mut self) -> Result<ServerCapabilities>;
    async fn list_tools(&self) -> Result<Vec<ToolDefinition>>;
    async fn call_tool(&self, name: &str, args: Value) -> Result<ToolResult>;
}
```

MCP 服务器在会话启动时初始化，通过 stdio 传输 JSON-RPC 消息。发现的工具自动注册到工具系统中，与内置工具统一暴露给 Agent。

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
    New,
    Resume,
    Agents,
    Clear,
    Compact(String),  // 参数: agent name
    Help,
    Quit,
}

impl SlashCommand {
    fn from_input(input: &str) -> Option<SlashCommand>;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    async fn execute(&self, ctx: &mut AppContext) -> Result<()>;
}
```

命令发现：输入 `/` 后，匹配所有命令名前缀，弹出补全列表。

#### 3.5.1 /compact 实现方案

`/compact <agent_name>` 使用指定 Agent 将当前会话历史压缩为一段摘要。

**流程：**
```txt
1. 取 session.messages 中除最后 N 条外的所有消息作为"待压缩区"（N 可配，默认保留最后 10 条）
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

[[messages]]
role = "assistant"
agent_name = "opus"
content = "考虑到高性能场景，推荐使用无锁环形缓冲区..."
created_at = "2026-02-28T14:30:32Z"
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
.krew/settings.toml           (项目级配置)
    ↓ 被覆盖
CLI 参数                      (命令行覆盖)
```

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
}

enum ApiType {
    Responses,  // OpenAI Responses API
    Chat,       // OpenAI Chat Completions API
}

#[derive(Deserialize)]
struct ProviderConfig {
    api_key: Option<String>,        // 方式二：直接填写（不推荐）
    api_key_env: Option<String>,    // 方式一：环境变量名（优先）
    base_url: Option<String>,
    azure_endpoint: Option<String>,     // Azure 模式: endpoint URL
    azure_api_version: Option<String>,  // Azure 模式: API 版本
}

#[derive(Deserialize)]
struct McpServerConfig {
    name: String,
    command: String,
    args: Vec<String>,
    env: Option<HashMap<String, String>>,  // 可选，默认空
    trust: Option<McpTrust>,    // auto | confirm (默认 confirm)
}

enum McpTrust {
    Auto,       // 跳过审批
    Confirm,    // 按 approval_mode 确认（默认）
}
```

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
 7. StreamEvent::Done
     │
     ▼
 8. 构建 Agent 回复 ChatMessage { role: Assistant, agent_name: "opus", ... }
     │
     ▼
 9. session.messages.push(reply)
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
├── Cargo.toml               # Workspace root
├── crates/
│   ├── krew-cli/             # 主 CLI 入口
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs       # 入口 + clap 解析
│   │       ├── app.rs        # App 状态机 + 主事件循环
│   │       └── render.rs     # TUI 渲染逻辑
│   │
│   ├── krew-core/            # 核心逻辑
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── session.rs    # Session 管理
│   │       ├── agent.rs      # Agent 运行时 + Agent Loop
│   │       ├── router.rs     # @ 寻址解析与消息路由
│   │       ├── message.rs    # 消息数据模型
│   │       └── command.rs    # Slash 命令定义与执行
│   │
│   ├── krew-llm/             # LLM Client 抽象
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs        # LlmClient trait
│   │       ├── openai_responses.rs    # OpenAI Responses API
│   │       ├── openai_chat.rs        # OpenAI Chat Completions API
│   │       ├── openai_compatible.rs  # OpenAI 兼容接口（豆包等，复用 openai 实现）
│   │       ├── anthropic.rs          # Anthropic 实现
│   │       ├── google.rs             # Google Gemini 实现
│   │       └── types.rs              # 统一类型定义
│   │
│   ├── krew-tools/           # 工具系统
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs        # Tool trait + 注册
│   │       ├── builtin/      # 内置工具
│   │       │   ├── mod.rs
│   │       │   ├── read_file.rs
│   │       │   ├── write_file.rs
│   │       │   ├── edit_file.rs
│   │       │   ├── shell.rs
│   │       │   ├── glob.rs
│   │       │   └── grep.rs
│   │       └── mcp.rs        # MCP 客户端
│   │
│   ├── krew-storage/         # 持久化
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       └── session_file.rs # TOML 会话文件读写
│   │
│   └── krew-config/          # 配置管理
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           └── defaults.rs   # 内置默认配置
│
├── docs/
│   ├── PDD.md
│   └── TDD.md
│
└── config.example.toml       # 示例配置文件
```

### 6.1 Crate 职责划分

| Crate | 职责 | 依赖 |
| ----- | ---- | ---- |
| `krew-cli` | CLI 入口、TUI 渲染、用户交互 | krew-core, krew-config |
| `krew-core` | 会话管理、Agent Loop、消息路由、Slash 命令 | krew-llm, krew-tools, krew-storage, krew-config |
| `krew-llm` | LLM API 抽象、各 Provider 实现 | reqwest, serde |
| `krew-tools` | 工具 trait、内置工具、MCP 客户端 | tokio, serde |
| `krew-storage` | TOML 文件会话持久化 | toml, serde |
| `krew-config` | TOML 配置加载与合并 | toml, serde |

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

```toml
[workspace]
resolver = "2"
members = [
    "crates/krew-cli",
    "crates/krew-core",
    "crates/krew-llm",
    "crates/krew-tools",
    "crates/krew-storage",
    "crates/krew-config",
]

# 版本号在开发启动时锁定为最新稳定版，以下为参考值
# 原则: default-features = false 最小化依赖，避免 dup crates
[workspace.dependencies]
tokio = { version = "1", default-features = false, features = ["rt-multi-thread", "macros", "net", "io-util", "time", "fs", "process", "signal"] }
serde = { version = "1", default-features = false, features = ["derive"] }
serde_json = { version = "1", default-features = false }
anyhow = { version = "1", default-features = false }
thiserror = { version = "2", default-features = false }
tracing = { version = "0.1", default-features = false }
tracing-subscriber = { version = "0.3", default-features = false, features = ["fmt"] }
chrono = { version = "0.4", default-features = false, features = ["serde", "clock"] }
uuid = { version = "1", default-features = false, features = ["v4"] }
toml = { version = "0.9", default-features = false, features = ["parse", "display"] }
reqwest = { version = "0.13", default-features = false, features = ["json", "stream", "rustls"] }
eventsource-stream = { version = "0.2", default-features = false }
futures = { version = "0.3", default-features = false }
```

### 7.2 各 Crate 特定依赖

**krew-cli**: `clap` (derive), `ratatui` (crossterm), `syntect`, `static_vcruntime` (Windows), `mimalloc` (Linux musl)
**krew-llm**: `reqwest`, `eventsource-stream`, `futures`
**krew-tools**: `tokio`, `globset`, `grep-regex`
**krew-storage**: `toml`, `serde`
**krew-config**: `toml`

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
| 消息模型 | SQ/EQ 队列 | 简化的 Vec<Message> |
| LLM 接入 | WebSocket + HTTP SSE | HTTP SSE（更简单） |
| TUI | Ratatui 全功能 | Ratatui 简化版 |
| 会话存储 | SQLite + JSONL | TOML 文件 |
| 工具系统 | MCP + 内置 + 沙箱 | MCP + 内置（无沙箱） |
| 配置 | TOML 多层 | TOML 单层（仅项目级） |
| 审批系统 | 多级策略 | 简化三级策略 |

---

## 附录 B: 后续迭代方向

- **v0.2**: Agent Skills 支持（为 Agent 配置特定技能/能力）
- **v0.3**: 自定义 Slash 命令（用户可扩展命令系统）
- **v0.4**: Agent 间可 @对方 形成 AI-to-AI 对话
