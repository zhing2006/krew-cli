# krew-cli — 产品设计文档 (PDD)

> 版本: 0.1.0 | 日期: 2026-02-28

---

## 1. 产品概述

### 1.1 愿景

krew-cli 是一个命令行多 AI Agent 协作会话工具。它允许用户在一个终端会话中同时与多个不同的 AI（如 GPT、Claude、Gemini）进行对话，就像组织一场"AI 圆桌会议"。用户可以向所有 AI 提问以获取多角度回答，也可以指定某个 AI 单独对话。

### 1.2 目标用户

- **任何人** — 利用多 AI 协作进行方案讨论、头脑风暴

### 1.3 核心价值

| 价值 | 描述 |
| ---- | ---- |
| **多模型协作** | 一个终端同时接入多个 LLM，无需切换工具 |
| **统一上下文** | 所有 Agent 共享完整会话历史，理解讨论全貌 |
| **精准寻址** | @all 广播 / @name 点名，灵活控制谁来回答 |
| **工具协作** | Agent 可调用工具（文件读写、Shell、MCP），协作完成实际任务 |
| **会话持久化** | 随时中断、随时恢复，讨论不丢失 |

---

## 2. 核心概念

### 2.1 Session（会话）

一次完整的多 Agent 对话过程。包含所有参与者的全部消息历史。每个 Session 有唯一 ID，可持久化到本地存储，支持 `/resume` 恢复和 `/new` 新建。

### 2.2 Agent（智能体）

一个接入会话的 AI 实例。每个 Agent 有：

- **name** — 唯一标识符，用于 @ 寻址（如 `gpt`、`opus`、`gemini`）
- **display_name** — 显示名称（如 `GPT-5.2`、`Claude Opus`）
- **provider** — 所属 LLM 提供商（openai / anthropic / google / openai-compatible）
- **api_type** — OpenAI 系专用：`responses` 或 `chat`（决定走哪个 API）
- **enable_web_search** — 是否启用模型原生 Web 搜索工具
- **model** — 具体模型 ID
- **system_prompt** — 可选的个性化系统提示词
- **color** — 终端显示颜色，用于区分不同 Agent 的发言
- **sampling** — 可选的采样参数（temperature、top_p、top_k、max_tokens 等），控制模型的生成行为。未设置时使用各模型默认值，max_tokens 默认取模型最大输出限制

### 2.3 Message（消息）

会话中的一条消息。包含：

- **role** — 发送者角色：`user`（用户）或 `assistant`（某个 Agent）
- **agent_name** — 当 role 为 assistant 时，标明是哪个 Agent 的发言
- **content** — 消息正文
- **tool_calls / tool_results** — 工具调用及结果（如有）
- **usage** — Agent 回复消息携带的 token 用量（输入/输出/总计）
- **created_at** — 消息创建时间
- **addressee** — 消息的目标寻址（`@all` / `@agent_name`）

### 2.4 Tool（工具）

Agent 可调用的能力扩展。分为：

- **内置工具** — 文件读写、Shell 执行、代码搜索等
- **MCP 工具** — 通过 MCP (Model Context Protocol) 接入的外部工具服务器

---

## 3. 用户故事

### US-1: 多模型对比问答

> 作为一个开发者，我希望同时向 GPT 和 Claude 提问同一个技术问题，对比它们的回答，以选择更好的方案。

```txt
you> @all 用 Rust 实现一个高性能的消息队列，应该选择什么数据结构？

[gpt] GPT-5.2:
  我建议使用 VecDeque 作为基础...

[opus] Claude Opus:
  考虑到高性能场景，推荐使用无锁环形缓冲区...
```

### US-2: 指定 Agent 深入讨论

> 作为一个用户，我觉得 Claude 的方案更好，想跟它继续深入讨论细节。

```txt
you> @opus 你提到的无锁环形缓冲区，能展开讲讲实现要点吗？

[opus] Claude Opus:
  无锁环形缓冲区的核心要点包括...
```

### US-3: AI 之间协作

> 作为一个用户，我希望让一个 AI 审查另一个 AI 生成的代码。

```txt
you> @gpt 请帮我写一个 Rust 的 HTTP server
[gpt] GPT-5.2:
  // ... 生成的代码 ...

you> @opus 请 review 一下 GPT 刚才写的代码
[opus] Claude Opus:
  我来审查这段代码，发现以下几个问题...
```

### US-4: 恢复历史会话

> 作为一个用户，我昨天和 AI 们讨论了一半的架构方案，今天想继续。

```txt
$ krew
krew> /resume
  [1] 2026-02-27 14:30 (gpt, opus) "用 Rust 实现一个高性能的消息队列..."
  [2] 2026-02-26 09:15 (gpt, gemini) "帮我对比一下前端框架..."
选择会话: 1
已恢复会话
krew> @all 我们昨天讨论到哪了？
```

### US-5: 工具协作

> 作为一个开发者，我希望 AI 能直接读取我的项目文件、执行命令来帮我解决问题。

```txt
you> @opus 帮我看看 src/main.rs 有什么问题

[opus] Claude Opus:
  [tool: read_file("src/main.rs")]
  我看了你的代码，第 42 行有一个潜在的内存泄漏...
```

---

## 4. 功能规格

### 4.1 多 Agent 会话机制

#### 4.1.1 @ 寻址语法

| 语法 | 行为 |
| ---- | ---- |
| `@all <message>` | 向所有 Agent 广播，按配置顺序依次回答 |
| `@<agent_name> <message>` | 仅指定 Agent 回答，其他 Agent 静默但可见该消息 |
| `<message>`（无 @ 前缀） | 发给上一个回答的 Agent（延续对话）；若无上一个回答者则提示用户指定 |

#### 4.1.2 @all 回答顺序

通过配置文件中的 `reply_order` 字段决定。各 Agent **按顺序串行执行**完整的 Agent Loop（包括工具调用），前一个 Agent 完成后才轮到下一个。

#### 4.1.3 消息上下文

所有 Agent 共享完整的会话历史。当用户 `@opus` 提问时，`gpt` 虽然不回答，但该消息及 `opus` 的回复会出现在所有 Agent 后续的上下文中。这确保每个 Agent 都能理解完整的讨论脉络。

**Agent 身份区分**：发送历史消息给某个 Agent 时，必须让它分清哪些是自己说的、哪些是其他 Agent 说的。其他 Agent 的回复会带有 `[agent_name] display_name:` 前缀标识。至于其他 Agent 的回复应以 user role 还是 assistant role 发送给 LLM，需要实际测试确定（详见 TDD 3.3.3）。

### 4.2 消息可见性与发言标识

#### 4.2.1 消息渲染格式

```txt
you> 用户的消息显示为 "you>" 前缀

[gpt] GPT-5.2:                      ← Agent 标识带颜色 + 显示名
  Agent 的回复内容，缩进显示
  支持 Markdown 渲染（代码块、列表等）

[opus] Claude Opus:
  另一个 Agent 的回复
```

#### 4.2.2 颜色区分

每个 Agent 在配置中指定一个终端颜色（如 green、blue、magenta），其标识标签 `[name]` 和显示名以该颜色渲染，确保视觉上可快速区分。

#### 4.2.3 工具调用显示

```txt
[opus] Claude Opus:
  ⚡ read_file("src/main.rs")       ← 工具调用，简洁显示
  看了你的代码后，我建议...
```

### 4.3 Slash 命令系统

| 命令 | 描述 |
| ---- | ---- |
| `/new` | 结束当前会话，开始新会话 |
| `/resume` | 列出历史会话，选择一个恢复 |
| `/agents` | 列出当前会话中的所有 Agent 及状态 |
| `/clear` | 清屏（不影响会话历史） |
| `/compact <agent>` | 压缩当前上下文（指定一个 Agent 总结之前的对话以释放 token） |
| `/help` | 显示所有可用命令 |
| `/quit` | 退出程序 |

Slash 命令以 `/` 开头，输入 `/` 时可弹出补全列表。

### 4.4 工具系统

#### 4.4.1 内置工具

| 工具 | 描述 |
| ---- | ---- |
| `read_file` | 读取文件内容 |
| `write_file` | 写入/创建文件 |
| `edit_file` | 编辑文件（基于搜索替换） |
| `shell` | 执行 Shell 命令 |
| `glob` | 文件模式搜索 |
| `grep` | 文件内容搜索 |

#### 4.4.2 MCP 集成

通过配置文件声明 MCP 服务器，Agent 可调用其提供的扩展工具：

```toml
[[mcp_servers]]
name = "github"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "$GITHUB_TOKEN" }
```

#### 4.4.3 工具审批

默认模式下，Agent 调用工具前需用户确认（特别是写文件、执行命令等有副作用的操作）。可通过配置调整审批策略：

| 策略 | 读操作 (read_file, glob, grep) | 写操作 (write_file, edit_file) | Shell 执行 | MCP 工具 |
| ---- | ---- | ---- | ---- | ---- |
| `suggest` | 自动 | 需确认 | 需确认 | 需确认 |
| `auto-edit` | 自动 | 自动 | 需确认 | 需确认 |
| `full-auto` | 自动 | 自动 | 自动 | 自动 |

### 4.5 会话历史管理

#### 4.5.1 自动持久化

每条消息实时写入本地存储（TOML 文件）。每个会话一个 `.toml` 文件，存储在 `.krew/sessions/` 目录下。即使意外退出也不会丢失历史。

#### 4.5.2 Token 用量追踪

每次 Agent 回复完成后，系统自动记录本次对话消耗的 token 数量（输入/输出），并累加到会话总计。用户可通过 `/agents` 命令查看各 Agent 的 token 用量统计。

每条 Agent 回复后，在输出末尾显示本次用量：

```txt
[opus] Claude Opus:
  基于分析，我的建议是...
                                          ── tokens: 2,847 in / 1,203 out
```

#### 4.5.3 自动压缩

当会话上下文的 token 数超过配置的阈值时（默认 120K tokens），系统在下一次对话开始前自动执行 `/compact`，压缩历史消息为摘要，释放上下文空间。

- 压缩前自动备份完整历史，确保可回滚
- 压缩完成后显示提示：`⚡ 会话已自动压缩 (N tokens → M tokens)`
- 可通过 `auto_compact_threshold = 0` 禁用自动压缩

#### 4.5.4 会话元数据

每个 Session 记录：

- 唯一 ID（同时作为文件名）
- 创建时间
- 最后活跃时间
- 参与的 Agent 列表
- 工作目录
- 累计 token 用量

#### 4.5.5 /resume 流程

1. 扫描 `.krew/sessions/` 下所有会话文件（按最后活跃时间倒序）
2. 显示摘要信息（时间、Agent 列表、第一条消息预览）
3. 用户选择一个会话
4. 加载完整消息历史到各 Agent 上下文
5. 继续对话

#### 4.5.6 /new 流程

1. 保存当前会话（如有）
2. 清空上下文
3. 开始全新会话

### 4.6 配置系统

#### 4.6.1 配置文件位置

| 位置 | 用途 |
| ---- | ---- |
| `.krew/settings.toml` | 项目级配置（Agent 定义、设置、Provider 配置） |
| `.krew/sessions/` | 会话数据（每个会话一个 .toml 文件） |
| `.krew/logs/` | 项目级日志 |

所有数据都存储在项目目录的 `.krew/` 下，不使用全局配置。

#### 4.6.2 配置文件结构

```toml
# 设置
[settings]
approval_mode = "suggest"              # 工具审批策略: suggest | auto-edit | full-auto
reply_order = ["gpt", "opus", "sonnet", "gemini", "doubao"] # @all 时的回答顺序
auto_compact_threshold = 120000        # 会话自动压缩 token 阈值（0 = 禁用）

# Agent 定义
[[agents]]
name = "gpt"
display_name = "GPT-5.2"
provider = "openai"
model = "gpt-5.2"
api_type = "responses"           # responses | chat，按模型选择
color = "green"
system_prompt = ""
tools = true
enable_web_search = false
# sampling.temperature = 0.7     # 可选：采样参数，未设置则使用模型默认值
# sampling.max_tokens = 32768    # 可选：最大输出 token，默认取模型最大值

[[agents]]
name = "opus"
display_name = "Claude Opus"
provider = "anthropic"
model = "claude-opus-4-6"
color = "magenta"
system_prompt = ""
tools = true
enable_web_search = false

[[agents]]
name = "sonnet"
display_name = "Claude Sonnet"
provider = "anthropic"
model = "claude-sonnet-4-6"
color = "cyan"
system_prompt = ""
tools = true
enable_web_search = false

[[agents]]
name = "gemini"
display_name = "Gemini 3.1 Pro"
provider = "google"
model = "gemini-3.1-pro"
color = "blue"
system_prompt = ""
tools = true
enable_web_search = false

[[agents]]
name = "doubao"
display_name = "Doubao Seed 2.0"
provider = "openai-compatible"    # 字节豆包走 OpenAI 兼容接口
model = "doubao-seed-2.0-pro"
color = "yellow"
system_prompt = ""
tools = true
enable_web_search = false

# Provider SDK 配置
[providers.openai]                      # OpenAI / Azure OpenAI
api_key_env = "OPENAI_API_KEY"
base_url = "https://api.openai.com/v1"
# azure_endpoint = "https://xxx.openai.azure.com"  # Azure 模式
# azure_api_version = "2025-01-01"                  # Azure API 版本

[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
base_url = "https://api.anthropic.com"

[providers.google]
api_key_env = "GOOGLE_API_KEY"

[providers.openai-compatible]           # 第三方 OpenAI 兼容服务
api_key_env = "DOUBAO_API_KEY"
base_url = "https://ark.cn-beijing.volces.com/api/v3"

# MCP 服务器
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
trust = "auto"                   # auto: 跳过审批 | confirm: 按审批策略确认（默认）
```

---

## 5. 交互设计

### 5.1 启动界面

```txt
 _                        _ _
| | ___ __ _____      __ | (_)
| |/ / '__/ _ \ \ /\ / / | | |
|   <| | |  __/\ V  V /  | | |
|_|\_\_|  \___| \_/\_/   |_|_|

krew v0.1.0 — Multi-Agent Meeting CLI
Agents: [gpt] GPT-5.2 | [opus] Claude Opus | [sonnet] Claude Sonnet | [gemini] Gemini 3.1 Pro | [doubao] Doubao Seed 2.0
Type /help for commands, @all or @name to start

you>
```

### 5.2 输入交互

- 支持多行输入（Shift+Enter 换行，Enter 发送）
- `@` 触发 Agent 名称补全
- `/` 触发 Slash 命令补全
- 上下箭头浏览历史输入
- Ctrl+C 中断当前 Agent 输出

### 5.3 输出格式

#### 普通对话

```txt
you> @all 你好，自我介绍一下

[gpt] GPT-5.2:
  你好！我是 GPT-5.2，由 OpenAI 开发的...

[opus] Claude Opus:
  你好！我是 Claude Opus，由 Anthropic 开发的...

[gemini] Gemini 3.1 Pro:
  你好！我是 Gemini 3.1 Pro，由 Google 开发的...
```

#### 工具调用

```txt
[opus] Claude Opus:
  ⚡ shell("cargo build 2>&1")
  ┌─────────────────────────────
  │ Compiling krew-cli v0.1.0
  │ Finished dev [unoptimized] in 2.3s
  └─────────────────────────────
  编译成功，没有错误。
```

#### 思考过程（如果模型支持）

```txt
[opus] Claude Opus:
  💭 让我分析一下这个问题...
  （思考过程折叠显示，可展开）

  基于分析，我的建议是...
```

### 5.4 流式输出

Agent 的回复以流式方式逐 token 渲染，用户可实时看到生成过程。当 `@all` 时，按 `reply_order` 顺序串行执行每个 Agent 的完整回合（含工具调用），前一个完成后下一个才开始。后续 Agent 可以看到前面 Agent 的回复。

---

## 6. 命令行参数

```txt
krew [OPTIONS]

Options:
  -c, --config <PATH>           指定配置文件路径
  -a, --agents <NAMES>          本次会话启用的 Agent（逗号分隔，覆盖配置）
      --approval-mode <MODE>    工具审批策略（suggest | auto-edit | full-auto）
      --resume [ID]             恢复指定会话（无 ID 则交互选择）
  -v, --verbose                 详细输出模式
  -h, --help                    帮助信息
  -V, --version                 版本信息
```

示例：

```bash
# 启动默认配置
krew

# 只启用两个 Agent
krew -a gpt,opus

# 恢复上次会话
krew --resume
```

---

## 7. 非功能性需求

### 7.1 性能

- 启动时间 < 500ms
- Agent 回复首 token 延迟取决于 LLM API，本地处理开销 < 50ms
- @all 串行执行各 Agent，每个 Agent 的流式输出实时渲染

### 7.2 安全

- API Key 支持环境变量引用（推荐）或直接配置，日志中不记录 Key 值
- Shell 执行和文件写入默认需用户确认
- **路径边界**：内置工具的文件操作限制在工作目录（cwd）及其子目录内，禁止访问 `..` 越界路径
- **命令安全**：`shell` 工具审批时完整显示待执行命令，用户可逐条确认
- **MCP 信任**：MCP 服务器调用的工具默认需要审批（同 Shell 级别），配置中可按 MCP server 名单独设置信任级别

### 7.3 可扩展性

- 新增 LLM Provider 只需实现统一 trait
- 通过 MCP 协议可扩展任意工具
- 项目级配置，CLI 参数可覆盖

### 7.4 兼容性

- 支持 Windows / macOS / Linux
- 支持常见终端模拟器（Windows Terminal、iTerm2、GNOME Terminal 等）
- 最低终端宽度 80 列

---

## 附录 A: 术语表

| 术语 | 定义 |
| ---- | ---- |
| Agent | 一个 AI 实例，绑定到特定 LLM Provider 和 Model |
| Session | 一次完整的多 Agent 会话，包含所有消息历史 |
| Provider | LLM 服务提供商（OpenAI / Anthropic / Google / OpenAI-Compatible） |
| MCP | Model Context Protocol，工具扩展协议 |
| Slash Command | 以 `/` 开头的命令，控制会话行为 |
| @ 寻址 | 以 `@` 开头的前缀，指定消息的目标 Agent |
