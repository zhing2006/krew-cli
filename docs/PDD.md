# krew-cli — 产品设计文档 (PDD)

> 版本: 0.7.0 | 日期: 2026-03-26

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
- **provider** — 引用的 Provider 配置名称（对应 `[providers.*]` 中的键名，如 `"openai"`、`"anthropic"`、`"doubao"`）
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

- **role** — 发送者角色：`user`（用户）、`assistant`（某个 Agent）或 `tool`（工具结果）
- **name** — 当 role 为 assistant 时，标明是哪个 Agent 的发言
- **content** — 消息正文
- **tool_calls** — Agent 发起的工具调用（如有）
- **tool_call_id** — 工具结果消息对应的调用 ID
- **server_tool_uses** — 服务端工具调用（如 Web Search）
- **usage** — Agent 回复消息携带的 token 用量（输入/输出/总计）
- **created_at** — 消息创建时间
- **addressee** — 消息的目标寻址（`@all` / `@agent_name`）
- **whisper_targets** — 密语目标 Agent 列表（设置时仅组内 Agent 可见消息内容）

### 2.4 Tool（工具）

Agent 可调用的能力扩展。分为：

- **内置工具** — 文件读写、图片查看、Shell 执行、代码搜索等
- **MCP 工具** — 通过 MCP (Model Context Protocol) 接入的外部工具服务器

---

## 3. 用户故事

### US-1: 多模型对比问答

> 作为一个开发者，我希望同时向 GPT 和 Claude 提问同一个技术问题，对比它们的回答，以选择更好的方案。

```txt
> ●●● @all 用 Rust 实现一个高性能的消息队列，应该选择什么数据结构？

[gpt] GPT-5.2:
  我建议使用 VecDeque 作为基础...

[opus] Claude Opus:
  考虑到高性能场景，推荐使用无锁环形缓冲区...
```

### US-2: 指定 Agent 深入讨论

> 作为一个用户，我觉得 Claude 的方案更好，想跟它继续深入讨论细节。

```txt
> ● @opus 你提到的无锁环形缓冲区，能展开讲讲实现要点吗？

[opus] Claude Opus:
  无锁环形缓冲区的核心要点包括...
```

### US-3: AI 之间协作

> 作为一个用户，我希望让一个 AI 审查另一个 AI 生成的代码。

```txt
> ● @gpt 请帮我写一个 Rust 的 HTTP server
[gpt] GPT-5.2:
  // ... 生成的代码 ...

> ● @opus 请 review 一下 GPT 刚才写的代码
[opus] Claude Opus:
  我来审查这段代码，发现以下几个问题...
```

### US-4: 密语（私密对话）

> 作为一个用户，我希望和特定 Agent 进行私密对话，不让其他 Agent 看到内容，例如让一个 AI 悄悄评价另一个 AI 的方案。

```txt
> ●●● @all 请给出你们的架构方案
[gpt] GPT-5.2:
  我建议使用微服务架构...
[opus] Claude Opus:
  我推荐单体优先，按需拆分...

> 🔒● #opus 你觉得 GPT 的方案有什么问题？

[opus] 🔒 Claude Opus:
  GPT 的微服务方案在当前阶段过度设计了...
```

密语组（`#a #b`）内的 Agent 可互相看到消息并通过 @mention 协作。密语消息在 TUI 中显示锁图标 🔒 标识。

### US-5: 恢复历史会话

> 作为一个用户，我昨天和 AI 们讨论了一半的架构方案，今天想继续。


```txt
$ krew
› /resume
  [1] 2026-02-27 14:30 (gpt, opus) "用 Rust 实现一个高性能的消息队列..."
  [2] 2026-02-26 09:15 (gpt, gemini) "帮我对比一下前端框架..."
选择会话: 1
已恢复会话
› @all 我们昨天讨论到哪了？
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
> ● @opus 帮我看看 src/main.rs 有什么问题

[opus] Claude Opus:
  ⚡ read_file("src/main.rs")
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

> **注：** `@name` / `#name` 可出现在消息的任意位置（不限于行首），消息正文保留完整原文不做剥离。`#all` 被禁止（解析器返回错误，提示不被支持）。`@` 和 `#` 不可混用（混合使用时报错）。

#### 4.1.2 @all 回答顺序

通过配置文件中的 `reply_order` 字段决定。各 Agent **按顺序串行执行**完整的 Agent Loop（包括工具调用），前一个 Agent 完成后才轮到下一个。

#### 4.1.3 消息上下文

所有 Agent 共享完整的会话历史。当用户 `@opus` 提问时，`gpt` 虽然不回答，但该消息及 `opus` 的回复会出现在所有 Agent 后续的上下文中。这确保每个 Agent 都能理解完整的讨论脉络。

**Agent 身份区分**：发送历史消息给某个 Agent 时，必须让它分清哪些是自己说的、哪些是其他 Agent 说的。其他 Agent 的回复会带有 `[agent_name] display_name:` 前缀标识。其他 Agent 的回复默认以 `user` role 发送，可通过 `settings.other_agent_role` 配置切换为 `assistant` role（详见 TDD 3.3.3）。

**Agent Identity Prompt**：每个 Agent 的 system prompt 中包含一段身份描述，告知其自身身份（名称、模型）、krew-cli 产品定位（多 AI Agent 协作 CLI 工具）、以及当用户需要修改配置时可执行 `krew config help` 获取配置手册。这使 Agent 能准确理解自己所处的协作环境并提供配置帮助。

#### 4.1.4 密语模式（# 语法）

`#name` 前缀发送密语（私密消息）。密语消息对组外 Agent 不可见，显示为 `[Whisper to name]` 占位符。

- **单目标密语**：`#opus hello` — 仅 opus 可见
- **多目标密语组**：`#opus #gemini discuss` — opus 和 gemini 互相可见消息，其他 Agent 看到占位符
- **密语回复继承**：Agent 对密语的回复自动继承相同的密语目标
- **组内 A2A**：密语组内的 Agent 可以互相 @mention，组外 mention 被忽略
- **`#all` 禁止**：解析器返回错误，提示 `#all` 不被支持
- **不继承模式**：不带 `#` 的后续消息回到普通模式（LastRespondent 不继承密语）
- **压缩保留**：密语消息在 `/compact` 时从压缩区提取并保留
- **视觉标识**：密语消息在 TUI 中显示 🔒 锁图标，P 模式显示 `[whisper]` 标记

### 4.2 消息可见性与发言标识

#### 4.2.1 消息渲染格式

```txt
> ●●● 用户的消息                     ← ">" 前缀（绿色粗体）+ 彩色路由圆点
                                      ← 每个圆点颜色对应目标 Agent 的配置颜色
> ● @opus 你觉得呢                   ← 单 Agent 时单个圆点
> 继续聊                             ← LastRespondent 时无圆点
> 🔒● #opus 私密消息                 ← 密语消息：锁图标 + 圆点

[gpt] GPT-5.2:                       ← Agent 回复：[name] 带颜色 + 显示名
  Agent 的回复内容，缩进显示
  支持 Markdown 渲染（代码块、列表等）

[opus] Claude Opus:
  另一个 Agent 的回复

[opus] 🔒 Claude Opus:               ← 密语回复：[name] + 锁图标 + 显示名
  密语模式下的回复
```

#### 4.2.2 颜色区分

每个 Agent 在配置中指定一个终端颜色（如 green、blue、magenta），其标识标签 `[name]` 以该颜色渲染，确保视觉上可快速区分。

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
| `/rewind` | 回退到历史消息，从该点重新开始对话（fork 语义） |
| `/agents` | 列出当前会话中的所有 Agent 及 token 用量统计 |
| `/compact <agent>` | 压缩当前上下文（指定一个 Agent 总结之前的对话以释放 token） |
| `/mcp` | 列出已连接的 MCP 服务器及其提供的工具 |
| `/skills` | 列出可用技能 |
| `/stats` | 显示进程统计信息（内存、线程数等） |
| `/help` | 显示所有可用命令 |
| `/exit` | 退出程序（别名 `/quit`） |

Slash 命令以 `/` 开头，输入 `/` 时可弹出补全列表（包含内置命令和自定义命令）。

### 4.4 工具系统

#### 4.4.1 内置工具

| 工具 | 描述 |
| ---- | ---- |
| `read_file` | 读取文件内容；支持图片文件（png/jpg/jpeg/gif/webp），自动识别并以多模态格式发送给 LLM |
| `write_file` | 写入/创建文件 |
| `edit_file` | 编辑文件（基于搜索替换） |
| `shell` | 执行 Shell 命令 |
| `glob` | 文件模式搜索 |
| `grep` | 文件内容搜索 |
| `fetch_url` | 抓取 URL 内容（HTTP 自动升级 HTTPS，HTML 转 Markdown，支持域名白名单免审批） |
| `activate_skill` | 激活指定 Skill，加载其完整指令（只读，发现 Skills 时自动注册） |

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

### 4.5 Skill 系统

krew 支持 Skill（技能）机制，为 Agent 提供任务相关的专业指令。Skill 以目录形式组织，包含一个 `SKILL.md` 文件定义技能的名称、描述和详细指令。

#### 4.5.1 Skill 发现

Skill 从多个目录自动发现，优先级从高到低：

| 优先级 | 路径 | 用途 |
| ------ | ---- | ---- |
| 1 | `.krew/skills/` | 项目级，krew 专属 |
| 2 | `.agents/skills/` | 项目级，跨客户端共享 |
| 3 | `.claude/skills/` | 项目级，Claude Code 兼容 |
| 4 | `~/.krew/skills/` | 用户级，krew 专属 |
| 5 | `~/.agents/skills/` | 用户级，跨客户端共享 |
| 6 | `~/.claude/skills/` | 用户级，Claude Code 兼容 |
| 7 | `skills.extra_paths` | 配置中指定的额外路径 |

扫描深度限制 4 层，跳过 `.git/`、`node_modules/`、`target/` 目录。同名 Skill 采用 first-found wins 策略。

#### 4.5.2 SKILL.md 格式

每个 Skill 目录下必须包含一个 `SKILL.md` 文件，使用 YAML frontmatter 定义元数据：

```markdown
---
name: code-review
description: Perform thorough code review with best practices
---

## Instructions
Review the code for...
```

必需字段：`name`、`description`。可选字段：`compatibility`、`metadata`。

Skill 目录可包含 `scripts/`、`references/`、`assets/` 等子目录作为资源文件。

#### 4.5.3 Skill Catalog 注入

系统启动时自动发现所有 Skill，构建 Skill Catalog 并注入到每个 Agent 的系统提示词中。Catalog 告知 Agent 可用的 Skill 列表及其描述，Agent 在遇到匹配任务时可调用 `activate_skill` 工具加载完整指令。

注入位置：项目指令（AGENTS.md）之后、Agent 自身 system_prompt 之前。

#### 4.5.4 Skill 激活

Agent 通过调用 `activate_skill` 工具加载 Skill 的完整指令内容。激活后返回 SKILL.md 的正文内容、Skill 目录绝对路径以及资源文件列表。同一会话中重复激活相同 Skill 会提示已存在。

#### 4.5.5 Skill 配置

```toml
[skills]
enabled = true                      # 是否启用 Skill 系统（默认 true）
extra_paths = ["/path/to/skills"]   # 额外的 Skill 搜索路径
```

### 4.6 自定义命令

用户可以通过 Markdown 文件定义自定义 Slash 命令，扩展命令系统。

#### 4.6.1 命令发现

自定义命令从以下目录发现（优先级从高到低）：

| 优先级 | 路径 |
| ------ | ---- |
| 1 | `.krew/commands/` |
| 2 | `.agents/commands/` |
| 3 | `.claude/commands/` |
| 4 | `~/.krew/commands/` |
| 5 | `~/.agents/commands/` |
| 6 | `~/.claude/commands/` |

递归扫描子目录，子目录形成命令名的命名空间（如 `commands/review/code.md` → `/review:code`）。同名命令采用 first-found wins 策略。内置命令优先级高于自定义命令。

#### 4.6.2 命令文件格式

```markdown
---
description: Review code for issues
argument-hint: <file_path>
---

Review the following file for potential issues: $ARGUMENTS
```

Frontmatter 字段（均可选）：
- `description` — 命令描述，显示在补全列表和 `/help` 中
- `argument-hint` — 参数提示，显示在补全列表中

#### 4.6.3 参数替换

- `$ARGUMENTS` — 替换为完整的参数字符串
- `$1`、`$2`、... — 替换为位置参数（按空格分割），未提供的位置参数替换为空字符串

#### 4.6.4 Bash 预处理

自定义命令内容支持 `` !`command` `` 语法，在参数替换之后、发送给 Agent 之前执行 Shell 命令，将 `` !`command` `` 块替换为命令的 stdout 输出。执行失败时替换为错误消息而非中止。

### 4.7 非交互式 Prompt 模式（-p）

通过 `-p <prompt>` 参数进入非交互式模式，执行单次 prompt 后退出，适用于脚本、CI/CD、管道组合等场景。

#### 4.7.1 基本用法

```bash
# 单 Agent
krew -p "@claude explain ownership in Rust"

# 多 Agent
krew -p "@claude @gpt compare your approaches"

# 广播所有 Agent
krew -p "@all hello"
```

#### 4.7.2 寻址要求

`-p` 模式下 prompt **必须**包含至少一个已知 `@agent`、`@all` 或 `#agent` 寻址，否则报错退出（exit code 2）。`#agent` 密语寻址使用与 TUI 相同的语义——标记 `whisper_targets` 并在 Agent 间执行可见性过滤。`#all` 被拒绝（exit code 2）。

#### 4.7.3 stdin 管道输入

支持 stdin 管道输入，内容以 `<stdin>...</stdin>` 标签包裹后拼接到 prompt 前方。寻址解析仅针对 `-p` 参数执行，stdin 中的 `@agent` 不影响路由。

```bash
# 管道输入文件内容
cat src/main.rs | krew -p "@opus review this"

# 管道输入命令输出
git diff | krew -p "@claude summarize these changes"
```

#### 4.7.4 输出格式

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

密语模式下 Text 格式 header 为 `[agent] [whisper]`。

**JSON 格式示例：**

```txt
{"agent":"claude","type":"tool_start","tool":"read_file","arguments":"{\"path\":\"src/main.rs\"}"}
{"agent":"claude","type":"tool_output","text":"compiling..."}
{"agent":"claude","type":"tool_done","tool":"read_file","summary":"done"}
{"agent":"claude","type":"server_tool_start","tool":"web_search"}
{"agent":"claude","type":"server_tool_done","tool":"web_search","query":"rust ownership"}
{"agent":"claude","type":"text","content":"这段代码有以下问题..."}
```

密语模式下 JSON 对象包含 `"whisper_targets": ["opus", "gemini"]` 字段。

**JSON 事件类型：**

| type | 说明 |
| ---- | ---- |
| `tool_start` | 工具调用开始 |
| `tool_output` | 工具实时输出（如 shell 流式输出） |
| `tool_done` | 工具调用完成 |
| `server_tool_start` | 服务端工具开始（如 Web Search） |
| `server_tool_done` | 服务端工具完成 |
| `text` | Agent 文本回复 |

#### 4.7.5 工具审批

`-p` 模式下强制使用 `full-auto` 审批策略，所有工具调用自动批准，无需用户确认。

#### 4.7.6 AI-to-AI 路由

支持 AI-to-AI 路由：当 Agent 回复中 @mention 其他 Agent 时，按配置的路由策略自动调度，遵循 `agent_to_agent_max_rounds` 限制。

#### 4.7.7 错误处理与 Exit Code

| Exit Code | 含义 |
| --------- | ---- |
| 0 | 所有 Agent 成功完成 |
| 1 | 任一 Agent 出错（如 API 错误） |
| 2 | 参数/配置错误（缺少寻址、参数冲突等） |

某个 Agent 出错时，继续执行队列中的下一个 Agent（与 TUI 模式一致）。错误信息输出到 stderr。

#### 4.7.8 会话持久化

`-p` 模式同样保存 session 到 `.krew/sessions/`，可通过 `--resume` 在 TUI 模式恢复。

#### 4.7.9 参数冲突

`-p` 与 `--resume` 不可同时使用，同时指定时报错退出（exit code 2）。

### 4.8 会话历史管理

#### 4.8.1 自动持久化

每条消息实时写入本地存储（TOML 文件）。每个会话一个 `.toml` 文件，存储在 `.krew/sessions/` 目录下。即使意外退出也不会丢失历史。

#### 4.8.2 Token 用量追踪

每次 Agent 回复完成后，系统自动记录本次对话消耗的 token 数量（输入/输出），并累加到会话总计。用户可通过 `/agents` 命令查看各 Agent 的 token 用量统计。

#### 4.8.3 自动压缩

当会话上下文的 token 数超过配置的阈值时（默认 120K tokens），系统在下一次对话开始前自动执行 `/compact`，压缩历史消息为摘要，释放上下文空间。

- 压缩前自动备份完整历史，确保可回滚
- 压缩完成后显示提示：`⚡ 会话已自动压缩 (N tokens → M tokens)`
- 可通过 `auto_compact_threshold = 0` 禁用自动压缩

#### 4.8.4 会话元数据

每个 Session 记录：

- 唯一 ID（同时作为文件名）
- 创建时间
- 最后活跃时间
- 参与的 Agent 列表
- 工作目录
- 累计 token 用量

#### 4.8.5 /resume 流程

1. 扫描 `.krew/sessions/` 下所有会话文件（按最后活跃时间倒序）
2. 显示摘要信息（时间、Agent 列表、第一条消息预览）
3. 用户选择一个会话
4. 加载完整消息历史到各 Agent 上下文
5. 继续对话

#### 4.8.6 /rewind 流程

1. 用户输入 `/rewind`，弹出 RewindPicker 弹窗
2. 列出所有用户消息（时间正序），用户选择回退点
3. 截断消息历史到选择点
4. 清屏并重放保留的消息
5. 设置 `rewound` 标记（fork 语义：不立即保存，发送新消息时生成新 session ID）
6. 选择第一条消息等同 `/clear`

#### 4.8.7 /new 流程

1. 保存当前会话（如有）
2. 清空上下文
3. 开始全新会话

### 4.9 Sub-Agent 系统（实验性）

Sub-Agent 是一种**上下文隔离**的子代理机制。当 Agent 需要执行专项任务（如 git 提交、代码调研）时，可以将任务委派给 Sub-Agent，在独立的上下文中执行，避免大量 tool call 消息污染主对话。

#### 4.9.1 定义格式

Sub-Agent 通过 Markdown 文件定义，格式兼容 Claude Code 的 `.claude/agents/*.md`：

```markdown
---
name: git
description: Git operations agent
color: cyan        # 可选，TUI 显示颜色
maxTurns: 50       # 可选，最大循环轮次（默认 30）
---

You are a git expert. Handle all git operations.
```

YAML frontmatter 中 `name` 和 `description` 为必需字段，body 作为 Sub-Agent 的 system prompt。Claude Code 的 `tools`、`model` 等字段会被解析但忽略。

#### 4.9.2 发现机制

系统从以下路径扫描 `*.md` 文件（按优先级从高到低，first-found-wins 去重）：

1. `<cwd>/.krew/agents/` — 项目级，krew-specific
2. `<cwd>/.agents/agents/` — 项目级，cross-client
3. `<cwd>/.claude/agents/` — 项目级，Claude Code 兼容
4. `<home>/.krew/agents/` — 用户级，krew-specific
5. `<home>/.agents/agents/` — 用户级，cross-client
6. `<home>/.claude/agents/` — 用户级，Claude Code 兼容

#### 4.9.3 调用方式

Agent 通过 `run_agent` tool 调用 Sub-Agent：

- 同步阻塞执行，Sub-Agent 完成后返回结果
- Sub-Agent 在完全隔离的上下文中运行（独立的消息历史）
- Sub-Agent 使用父 Agent 的所有工具（含 MCP），但不能嵌套调用 `run_agent`
- Sub-Agent 的 tool 调用过程实时流式展示给用户
- Sub-Agent 遵守父 Agent 的审批配置

#### 4.9.4 功能开关

通过 `sub_agent_enabled = true` 启用（默认关闭）。关闭时完全不读取 agent 定义文件、不注册 tool、零开销。

### 4.10 配置系统

#### 4.10.1 配置文件位置

| 位置 | 用途 |
| ---- | ---- |
| `~/.krew/settings.toml` | 用户级配置（providers、偏好设置、全局 MCP） |
| `.krew/settings.toml` | 项目级配置（Agent 定义、设置、Provider 配置） |
| `.krew/sessions/` | 会话数据（每个会话一个 .toml 文件） |
| `.krew/logs/` | 项目级日志 |

支持两层配置：用户级（`~/.krew/settings.toml`）和项目级（`.krew/settings.toml`）。用户级配置提供 providers、API keys、偏好设置等跨项目共享配置；项目级配置定义 agents 和项目特有覆盖。合并规则：project 覆盖 user（同名 provider 整项替换，同名 MCP server 替换，标量字段 project 优先）。`agents` 和 `reply_order` 仅在项目级配置中定义。

#### 4.9.1.1 Commands / Skills 多目录 Discovery

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

#### 4.9.2 配置文件结构

```toml
# 设置
[settings]
approval_mode = "suggest"              # 工具审批策略: suggest | auto-edit | full-auto
reply_order = ["gpt", "opus", "sonnet", "gemini", "doubao"] # @all 时的回答顺序
auto_compact_threshold = 120000        # 会话自动压缩 token 阈值（0 = 禁用）
compact_keep_rounds = 10               # 压缩时保留最近 N 轮对话（默认 10）
# other_agent_role = "user"            # 其他 Agent 消息的 role: user | assistant
# worker_threads = 4                   # tokio 工作线程数
# shell_allow_commands = ["ls", "cargo", "git status"]  # 免审批 shell 命令前缀
# fetch_allow_domains = ["docs.rs"]    # 免审批 fetch_url 域名白名单
# agent_to_agent_routing = "immediate" # AI-to-AI 路由策略: immediate | queued（默认 immediate）
# agent_to_agent_max_rounds = 10       # AI-to-AI 最大轮次（0 = 禁用，默认 10）
# restrict_workspace = true            # 限制内建文件工具只能访问工作区目录（默认 true）

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

# Skill 配置
[skills]
enabled = true                         # 是否启用 Skill 系统（默认 true）
# extra_paths = ["/path/to/skills"]    # 额外的 Skill 搜索路径
```

#### 4.9.3 项目级指令文件（AGENTS.md）

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

启动 banner 为三行内容加边框（共 5 行）：

```txt
┌──────────────────────────────────────────────────────────────────────────┐
│ Krew CLI v0.6.0                                                        │
│ Agents: [gpt] GPT-5.2 | [opus] Claude Opus | [gemini] Gemini 3.1 Pro  │
│ Directory: H:\ZHing\...ew-cli          Type /help for commands         │
└──────────────────────────────────────────────────────────────────────────┘
›
```

- 第一行：标题 + 版本
- 第二行：Agent 列表（按 `reply_order` 排序，`[name]` 带颜色）
- 第三行：左对齐 `Directory:` + 工作目录路径（超宽时中间截断 `...`），右对齐 `Type /help for commands`
- 状态栏显示当前 `approval_mode` 和 `auto_compact_threshold`（如 `suggest | auto-compact 120k`）
- 输入提示符为 `›`

### 5.2 输入交互

- 支持多行输入（Shift+Enter / Ctrl+J 换行，Enter 发送）
- `@` 触发 Agent 名称补全弹窗（含 "all"）
- `#` 触发密语目标补全弹窗（不含 "all"）
- `/` 触发 Slash 命令补全弹窗（含内置命令和自定义命令）
- 上下箭头浏览历史输入（输入历史持久化到 `.krew/history`，跨 session 保留，换行转义为 `\n`，启动时按 `input_history_limit` 截断回写，`/new` 和 `/resume` 不清除历史）
- ESC 取消当前 Agent 流式输出
- 双击 Ctrl+C 退出程序

### 5.3 输出格式

#### 普通对话

```txt
> ●●● @all 你好，自我介绍一下

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
  │ Compiling krew-cli v0.6.0
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

#### Agent 状态指示器

Agent 生成回复期间，状态栏显示动态指示器：

```txt
● Claude Opus Working 45s — ESC to interrupt
```

包含闪烁的 spinner（`●`/`◦` 交替）、Agent 显示名、"Working" 文字、已用时间和中断提示。回复完成后指示器消失。

### 5.4 流式输出

Agent 的回复以流式方式逐 token 渲染（~60Hz 刷新），用户可实时看到生成过程。当 `@all` 时，按 `reply_order` 顺序串行执行每个 Agent 的完整回合（含工具调用），前一个完成后下一个才开始。后续 Agent 可以看到前面 Agent 的回复。

---

### 4.10 配置管理（`krew config`）

krew 提供一套 `config` 子命令用于交互式配置管理，无需手动编辑 TOML 文件。

#### 4.10.1 交互式初始化（`krew config init`）

```bash
krew config init              # 智能路由：根据配置文件存在情况自动决定
krew config init --user       # 仅设置用户级配置
krew config init --project    # 仅设置项目级配置
```

**用户配置初始化**：循环式 Provider 创建向导（选择类型 → 输入名称 → 选择 API Key 方式 → 可选参数 → OpenAI 兼容 Provider 选择 API 类型：Chat Completions 或 Responses API）。如果 `base_url` 以 `/v1` 结尾，自动去掉以避免路径重复。

**项目配置初始化**：提供两种模式：
- **智能预设**：从已配置的 Provider 获取可用模型列表（调用 List Models API），提供单 Agent 或三 Agent 预设方案，通过模糊搜索选择模型
- **手动设置**：循环式 Agent 创建——选择 Provider、模型、名称、显示名、颜色、Thinking、Web Search；`tools` 默认启用

#### 4.10.2 增删管理（`krew config add/del`）

```bash
krew config add provider    # 添加 Provider 到用户配置
krew config add agent       # 添加 Agent 到项目配置
krew config del provider    # 删除 Provider（带依赖检查）
krew config del agent       # 删除 Agent（带最后一个 Agent 警告）
```

使用 `toml_edit` 实现格式保留写入，现有注释和格式不受影响。

#### 4.10.3 查看与诊断（`krew config list/doctor/help`）

```bash
krew config list providers   # 表格显示 Provider（含 API Key 状态 ✅/❌）
krew config list agents      # 表格显示 Agent 配置
krew config doctor           # 诊断配置完整性（文件语法、Key 可用性、引用关系、MCP 命令）
krew config help             # 打印完整配置手册
```

所有 `config` 子命令在普通终端模式下运行（不启动 TUI），不加载完整 Config（仅按需读取用户/项目配置文件）。

---

## 6. 命令行参数

```txt
krew [OPTIONS] [COMMAND]

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

Commands:
  config init [--user|--project]   交互式配置初始化
  config add <provider|agent>      添加 Provider 或 Agent
  config del <provider|agent>      删除 Provider 或 Agent
  config list <providers|agents>   列出 Provider 或 Agent
  config doctor                    诊断配置问题
  config help                      打印完整配置手册
```

示例：

```bash
# 交互式配置初始化
krew config init

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
| Skill | 以 SKILL.md 定义的可激活技能，为 Agent 提供任务专业指令 |
| Custom Command | 用户自定义的 Slash 命令，以 Markdown 文件定义 |
| Slash Command | 以 `/` 开头的命令，控制会话行为（含内置命令和自定义命令） |
| @ 寻址 | 以 `@` 开头的前缀，指定消息的目标 Agent |
| # 密语 | 以 `#` 开头的前缀，向指定 Agent 发送私密消息 |
| 密语组 | 多目标密语（`#a #b`）的成员集合，组内互相可见 |
| Rewind | 回退到历史消息点重新开始对话，使用 fork 语义 |
