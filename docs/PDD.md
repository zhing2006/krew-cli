# krew-cli — 产品设计文档 (PDD)

> 版本: 0.1.0 | 日期: 2026-03-06

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
| **密语对话** | #name 私密消息，其他 Agent 不可见，支持多目标密语组 |
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
- **enable_thinking** — 是否启用思考/推理过程输出
- **thinking_effort** — 思考力度（low / medium / high）
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
- **whisper_targets** — 密语目标 Agent 列表（设置时仅组内 Agent 可见消息内容）

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

### US-4: 密语（私密对话）

> 作为一个用户，我希望和特定 Agent 进行私密对话，不让其他 Agent 看到内容，例如让一个 AI 悄悄评价另一个 AI 的方案。

```txt
you> @all 请给出你们的架构方案
[gpt] GPT-5.2:
  我建议使用微服务架构...
[opus] Claude Opus:
  我推荐单体优先，按需拆分...

you> #opus 你觉得 GPT 的方案有什么问题？
🔒●  #opus 你觉得 GPT 的方案有什么问题？

[opus] 🔒 Claude Opus:
  GPT 的微服务方案在当前阶段过度设计了...
```

密语组（`#a #b`）内的 Agent 可互相看到消息并通过 @mention 协作。密语消息在 TUI 中显示锁图标 🔒 标识。

### US-5: 恢复历史会话

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

### US-6: 非交互式 Prompt 模式

> 作为一个开发者，我希望在脚本或 CI 中调用 krew，无需手动交互即可获取 AI 回复。

```txt
# 单 Agent 调用
$ krew -p "@claude 解释一下 Rust 的所有权机制"
[claude]
Rust 的所有权机制是其内存安全的核心...

# 管道输入 + 代码审查
$ cat src/main.rs | krew -p "@opus review this code"
[opus]
⚡ read_file(src/main.rs)
   ⎿  done

我看了你的代码，建议以下改进...

# JSON 格式输出（适合脚本解析）
$ krew -p "@all hello" --format json
{"agent":"gpt","type":"text","content":"Hello! I'm GPT..."}
{"agent":"opus","type":"text","content":"Hello! I'm Claude..."}
```

### US-7: 工具协作

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
| `@<name1> @<name2> <message>` | 指定多个 Agent 依次回答（按 `@` 出现顺序） |
| `<message>`（无 `@` 前缀） | 发给上一个回答的 Agent（延续对话）；若无上一个回答者则提示用户指定 |
| `#<agent_name> <message>` | 向指定 Agent 发送密语（私密消息），其他 Agent 仅看到占位符 |
| `#<name1> #<name2> <message>` | 多目标密语：组内成员互相可见消息，组外 Agent 看到占位符 |

> **注：** `@name` / `#name` 可出现在消息的任意位置（不限于行首），消息正文保留完整原文不做剥离。`#all` 被禁止（语义等同普通消息）。`@` 和 `#` 不可混用。

#### 4.1.2 @all 回答顺序

通过配置文件中的 `reply_order` 字段决定。各 Agent **按顺序串行执行**完整的 Agent Loop（包括工具调用），前一个 Agent 完成后才轮到下一个。

#### 4.1.3 消息上下文

所有 Agent 共享完整的会话历史。当用户 `@opus` 提问时，`gpt` 虽然不回答，但该消息及 `opus` 的回复会出现在所有 Agent 后续的上下文中。这确保每个 Agent 都能理解完整的讨论脉络。

**Agent 身份区分**：发送历史消息给某个 Agent 时，必须让它分清哪些是自己说的、哪些是其他 Agent 说的。其他 Agent 的回复会带有 `[agent_name] display_name:` 前缀标识。至于其他 Agent 的回复应以 user role 还是 assistant role 发送给 LLM，需要实际测试确定（详见 TDD 3.3.3）。

#### 4.1.4 密语模式（# 语法）

`#name` 前缀发送密语（私密消息）。密语消息对组外 Agent 不可见，显示为 `[Whisper to name]` 占位符。

- **单目标密语**：`#opus hello` — 仅 opus 可见
- **多目标密语组**：`#opus #gemini discuss` — opus 和 gemini 互相可见消息，其他 Agent 看到占位符
- **密语回复继承**：Agent 对密语的回复自动继承相同的密语目标
- **组内 A2A**：密语组内的 Agent 可以互相 @mention，组外 mention 被忽略
- **`#all` 禁止**：对所有 Agent 密语等同于普通消息，解析器拒绝
- **不继承模式**：不带 `#` 的后续消息回到普通模式（LastRespondent 不继承密语）
- **压缩保留**：密语消息在 `/compact` 时从压缩区提取并保留
- **视觉标识**：密语消息在 TUI 中显示 🔒 锁图标，P 模式显示 `[whisper]` 标记

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
| `/clear` | 清屏并开始新会话（别名 `/new`） |
| `/resume` | 列出历史会话，选择一个恢复 |
| `/agents` | 列出当前会话中的所有 Agent 及 token 用量统计 |
| `/compact <agent>` | 压缩当前上下文（指定一个 Agent 总结之前的对话以释放 token） |
| `/mcp` | 列出已连接的 MCP 服务器及其提供的工具 |
| `/skills` | 列出可用技能 |
| `/stats` | 显示进程统计信息（内存、线程数等） |
| `/help` | 显示所有可用命令 |
| `/exit` | 退出程序（别名 `/quit`） |

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
| `fetch_url` | 抓取 URL 内容（HTML 自动转换为 Markdown，支持域名白名单免审批） |

#### 4.4.2 MCP 集成

通过配置文件声明 MCP 服务器，Agent 可调用其提供的扩展工具。支持 stdio（子进程）和 Streamable HTTP 两种传输：

```toml
# Stdio 传输（子进程）
[[mcp_servers]]
name = "github"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = "$GITHUB_TOKEN" }

# HTTP 传输
[[mcp_servers]]
name = "remote-tools"
url = "https://mcp.example.com/sse"
headers = { Authorization = "Bearer $TOKEN" }
```

#### 4.4.3 工具审批

默认模式下，Agent 调用工具前需用户确认（特别是写文件、执行命令等有副作用的操作）。可通过配置调整审批策略：

| 策略 | 读操作 (read_file, glob, grep) | 写操作 (write_file, edit_file) | Shell 执行 | fetch_url | MCP 工具 |
| ---- | ---- | ---- | ---- | ---- | ---- |
| `suggest` | 自动 | 需确认 | 需确认* | 白名单域名自动，其他需确认 | 需确认 |
| `auto-edit` | 自动 | 自动 | 需确认* | 白名单域名自动，其他需确认 | 需确认 |
| `full-auto` | 自动 | 自动 | 自动 | 自动 | 自动 |

*Shell 命令可通过 `shell_allow_commands` 配置前缀匹配的免审批列表（如 `["ls", "cargo build", "git status"]`）。

### 4.5 非交互式 Prompt 模式（-p）

通过 `-p <prompt>` 参数进入非交互式模式，执行单次 prompt 后退出，适用于脚本、CI/CD、管道组合等场景。

#### 4.5.1 基本用法

```bash
# 单 Agent
krew -p "@claude explain ownership in Rust"

# 多 Agent
krew -p "@claude @gpt compare your approaches"

# 广播所有 Agent
krew -p "@all hello"
```

#### 4.5.2 寻址要求

`-p` 模式下 prompt **必须**包含至少一个已知 `@agent` 或 `@all` 寻址，否则报错退出（exit code 2）。这是因为非交互式模式没有"上一个回答者"的概念。

#### 4.5.3 stdin 管道输入

支持 stdin 管道输入，内容以 `<stdin>...</stdin>` 标签包裹后拼接到 prompt 前方。寻址解析仅针对 `-p` 参数执行，stdin 中的 `@agent` 不影响路由。

```bash
# 管道输入文件内容
cat src/main.rs | krew -p "@opus review this"

# 管道输入命令输出
git diff | krew -p "@claude summarize these changes"
```

#### 4.5.4 输出格式

| 格式 | 参数 | 描述 |
| ---- | ---- | ---- |
| Text（默认） | `--format text` | Streaming 输出，`[agent_name]` header + 逐 token 打印 |
| JSON | `--format json` | JSONL 格式，每行一个 JSON 对象，非 streaming |

**Text 格式示例：**

```txt
[claude]
⚡ read_file(src/main.rs)
   ⎿  done

这段代码有以下问题...
```

**JSON 格式示例：**

```txt
{"agent":"claude","type":"tool_start","tool":"read_file","arguments":"{\"path\":\"src/main.rs\"}"}
{"agent":"claude","type":"tool_done","tool":"read_file","summary":"done"}
{"agent":"claude","type":"text","content":"这段代码有以下问题..."}
```

#### 4.5.5 工具审批

`-p` 模式下强制使用 `full-auto` 审批策略，所有工具调用自动批准，无需用户确认。

#### 4.5.6 AI-to-AI 路由

支持 AI-to-AI 路由：当 Agent 回复中 @mention 其他 Agent 时，按配置的路由策略自动调度，遵循 `agent_to_agent_max_rounds` 限制。

#### 4.5.7 错误处理与 Exit Code

| Exit Code | 含义 |
| --------- | ---- |
| 0 | 所有 Agent 成功完成 |
| 1 | 任一 Agent 出错（如 API 错误） |
| 2 | 参数/配置错误（缺少寻址、参数冲突等） |

某个 Agent 出错时，继续执行队列中的下一个 Agent（与 TUI 模式一致）。错误信息输出到 stderr。

#### 4.5.8 会话持久化

`-p` 模式同样保存 session 到 `.krew/sessions/`，可通过 `--resume` 在 TUI 模式恢复。

#### 4.5.9 参数冲突

`-p` 与 `--resume` 不可同时使用，同时指定时报错退出（exit code 2）。

### 4.6 会话历史管理

#### 4.5.1 自动持久化

每条消息实时写入本地存储（TOML 文件）。每个会话一个 `.toml` 文件，存储在 `.krew/sessions/` 目录下。即使意外退出也不会丢失历史。

#### 4.5.2 Token 用量追踪

每次 Agent 回复完成后，系统自动记录本次对话消耗的 token 数量（输入/输出），并累加到会话总计。用户可通过 `/agents` 命令查看各 Agent 的 token 用量统计。

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
| `~/.krew/settings.toml` | 用户级配置（providers、偏好设置、全局 MCP） |
| `.krew/settings.toml` | 项目级配置（Agent 定义、设置、Provider 配置） |
| `.krew/sessions/` | 会话数据（每个会话一个 .toml 文件） |
| `.krew/logs/` | 项目级日志 |

支持两层配置：用户级（`~/.krew/settings.toml`）和项目级（`.krew/settings.toml`）。用户级配置提供 providers、API keys、偏好设置等跨项目共享配置；项目级配置定义 agents 和项目特有覆盖。合并规则：project 覆盖 user（同名 provider 整项替换，同名 MCP server 替换，标量字段 project 优先）。`agents` 和 `reply_order` 仅在项目级配置中定义。

#### 4.6.1.1 Commands / Skills 多目录 Discovery

自定义命令和 Skills 支持从多个目录发现，优先级从高到低：

| 优先级 | 路径 | 用途 |
| ------ | ---- | ---- |
| 1 | `.krew/commands/` · `.krew/skills/` | 项目级，krew 专属 |
| 2 | `.agents/commands/` · `.agents/skills/` | 项目级，跨客户端共享 |
| 3 | `.claude/commands/` · `.claude/skills/` | 项目级，Claude Code 兼容 |
| 4 | `~/.krew/commands/` · `~/.krew/skills/` | 用户级，krew 专属 |
| 5 | `~/.agents/commands/` · `~/.agents/skills/` | 用户级，跨客户端共享 |
| 6 | `~/.claude/commands/` · `~/.claude/skills/` | 用户级，Claude Code 兼容 |

同名条目采用 first-found wins 策略。

#### 4.6.2 配置文件结构

```toml
# 设置
[settings]
approval_mode = "suggest"              # 工具审批策略: suggest | auto-edit | full-auto
reply_order = ["gpt", "opus", "sonnet", "gemini", "doubao"] # @all 时的回答顺序
auto_compact_threshold = 120000        # 会话自动压缩 token 阈值（0 = 禁用）
compact_keep_rounds = 3                # 压缩时保留最近 N 轮对话
# other_agent_role = "user"            # 其他 Agent 消息的 role: user | assistant
# worker_threads = 4                   # tokio 工作线程数
# shell_allow_commands = ["ls", "cargo", "git status"]  # 免审批 shell 命令前缀
# fetch_allow_domains = ["docs.rs"]    # 免审批 fetch_url 域名白名单
# agent_to_agent_routing = "immediate" # AI-to-AI 路由策略: immediate | queued（默认 immediate）
# agent_to_agent_max_rounds = 10       # AI-to-AI 最大轮次（0 = 禁用，默认 10）

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
enable_thinking = false          # 启用思考/推理输出
# thinking_effort = "medium"     # 思考力度: low | medium | high
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
[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"
base_url = "https://api.openai.com"

[providers.anthropic]
type = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
base_url = "https://api.anthropic.com"

[providers.google]
type = "google"
api_key_env = "GOOGLE_API_KEY"

[providers.openai-compatible]           # 第三方 OpenAI 兼容服务（type 仍为 "openai"）
type = "openai"
api_key_env = "DOUBAO_API_KEY"
base_url = "https://ark.cn-beijing.volces.com/api/v3"

# MCP 服务器（Stdio 传输）
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
trust = "auto"                   # auto: 跳过审批 | confirm: 按审批策略确认（默认）

# MCP 服务器（HTTP 传输）
# [[mcp_servers]]
# name = "remote-tools"
# url = "https://mcp.example.com/sse"
# headers = { Authorization = "Bearer $TOKEN" }
```

#### 4.6.3 项目级指令文件（AGENTS.md）

krew 支持通过项目目录中的 `AGENTS.md` 文件为所有 Agent 提供项目上下文（架构说明、编码规范、工作约定等）。该机制类似于 Claude Code 的 `CLAUDE.md` 和 OpenAI Codex 的 `AGENTS.md`。

**文件位置与发现规则：**

- 文件名固定为 `AGENTS.md`，放置在项目目录（工作目录）下
- 支持层级化加载：krew 启动时从工作目录开始向上遍历父目录，收集所有找到的 `AGENTS.md` 文件
- 合并顺序：祖先目录在前，子目录在后（子目录内容可补充或覆盖祖先的通用指令）
- 该文件为可选项，不存在时静默跳过

**注入行为：**

- 加载的指令内容会自动注入到所有 Agent 的系统提示词中（在 `system_prompt` 之前）
- 使用 `<project-instructions>` 标签包裹，与 Agent 个性化提示词区分
- 单个文件大小限制 100KB，超出部分会被截断

**示例：**

```markdown
# AGENTS.md

## 项目架构
本项目使用 Rust + tokio 异步运行时...

## 编码规范
- 使用 snake_case 命名
- 所有 pub 函数需要文档注释
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
- `@` 触发 Agent 名称补全弹窗（含 "all"）
- `#` 触发密语目标补全弹窗（不含 "all"）
- `/` 触发 Slash 命令补全弹窗
- 上下箭头浏览历史输入（输入历史持久化到 `.krew/history`）
- ESC 取消当前 Agent 流式输出
- Ctrl+C 退出程序

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
  -p, --prompt <PROMPT>         非交互式 prompt 模式，执行后退出
      --format <FORMAT>         prompt 模式输出格式（text | json，默认 text）
  -v, --verbose                 详细输出模式
  -h, --help                    帮助信息
  -V, --version                 版本信息
```

示例：

```bash
# 启动默认配置（TUI 交互模式）
krew

# 只启用两个 Agent
krew -a gpt,opus

# 恢复上次会话
krew --resume

# 非交互式 prompt 模式
krew -p "@claude explain this code"

# 管道输入 + JSON 输出
cat src/main.rs | krew -p "@opus review" --format json
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

- 支持 Windows x64 / macOS x64+arm64 / Linux x64+arm64（五平台静态链接二进制）
- 通过 `npm install -g @zhing2026/krew` 或 GitHub Release 下载安装
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
| # 密语 | 以 `#` 开头的前缀，向指定 Agent 发送私密消息 |
| 密语组 | 多目标密语（`#a #b`）的成员集合，组内互相可见 |
