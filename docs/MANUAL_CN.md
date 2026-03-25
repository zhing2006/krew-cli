# krew-cli 使用手册

> 版本: 0.6.0

---

## 目录

1. [安装](#1-安装)
2. [快速开始](#2-快速开始)
3. [常用操作指南](#3-常用操作指南)
4. [命令行参数](#4-命令行参数)
5. [配置文件](#5-配置文件)
6. [配置管理（`krew config`）](#6-配置管理krew-config)
7. [寻址与路由](#7-寻址与路由)
8. [Slash 命令](#8-slash-命令)
9. [自定义命令](#9-自定义命令)
10. [工具系统](#10-工具系统)
11. [MCP 集成](#11-mcp-集成)
12. [Skill 系统](#12-skill-系统)
13. [会话管理](#13-会话管理)
14. [Prompt 模式](#14-prompt-模式)
15. [项目指令 (AGENTS.md)](#15-项目指令-agentsmd)
16. [文件路径与加载优先级](#16-文件路径与加载优先级)
17. [快捷键](#17-快捷键)
18. [常见问题](#18-常见问题)

---

## 1. 安装

### npm（推荐）

```bash
npm install -g @zhing2026/krew
```

### GitHub Releases

从 [GitHub Releases](https://github.com/ZHing2006/krew-cli/releases) 下载对应平台的二进制文件：

| 平台 | 文件名 |
| ---- | ------ |
| Windows x64 | `krew-win32-x64.exe` |
| Linux x64 | `krew-linux-x64` |
| Linux arm64 | `krew-linux-arm64` |
| macOS x64 | `krew-darwin-x64` |
| macOS arm64 | `krew-darwin-arm64` |

所有二进制文件均为静态链接，无需额外依赖。

### 从源码构建

需要 Rust（edition 2024）和 Cargo：

```bash
git clone https://github.com/ZHing2006/krew-cli.git
cd krew-cli
cargo install --path crates/krew-cli
```

### 验证安装

```bash
krew --version
```

---

## 2. 快速开始

本节带你完成第一次 krew 对话。

### 方式 A：交互式配置（推荐）

最快的上手方式——配置向导帮你搞定一切：

```bash
krew config init
```

向导会：
1. 引导你设置 Provider（选择类型、输入 API Key）
2. 引导你定义 Agent（选择 Provider、选择模型）
3. 自动写入 `~/.krew/settings.toml`（用户级）和 `.krew/settings.toml`（项目级）

然后直接运行 `krew` 即可开始对话！

### 方式 B：手动配置

#### 第一步：创建配置目录

**macOS / Linux：**
```bash
mkdir -p .krew
```

**Windows (PowerShell)：**
```powershell
mkdir .krew -Force
```

#### 第二步：创建 `.krew/settings.toml`

在项目目录下创建 `.krew/settings.toml`。以下是最简配置——一个 Agent、一个 Provider：

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

#### 第三步：设置 API Key

**macOS / Linux：**
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
```

**Windows (PowerShell)：**
```powershell
$env:ANTHROPIC_API_KEY = "sk-ant-..."
```

**Windows (CMD)：**
```cmd
set ANTHROPIC_API_KEY=sk-ant-...
```

> **提示：** 将 export 命令加入 shell 配置文件（`~/.bashrc`、`~/.zshrc` 或 Windows 系统环境变量），这样不用每次都设置。

#### 第四步：启动

```bash
krew
```

你会看到这样的启动界面：

```
┌──────────────────────────────────────────────────┐
│ Krew CLI v0.6.0                                  │
│ Agents: [opus] Claude Opus                       │
│ Directory: /path/to/project  Type /help for ...  │
└──────────────────────────────────────────────────┘
›
```

输入 `@opus 你好！` 然后按 Enter，开始对话！

### 接下来可以做什么？

- 添加更多 Agent → `krew config add agent` 或看[指南 1：多 Provider 配置](#指南-1多-provider-配置openai--anthropic)
- 试试密语模式 → 输入 `#opus 悄悄话`
- 使用工具 → 让 Agent 读取文件或执行命令
- 诊断配置问题 → `krew config doctor`
- 查看所有命令 → 在会话中输入 `/help`

---

## 3. 常用操作指南

### 指南 1：多 Provider 配置（OpenAI + Anthropic）

设置两个不同 Provider 的 Agent，对比回答。

**1. 设置 API Key：**

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

**2. 创建 `.krew/settings.toml`：**

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

**3. 试一下：**

```
› @all Rust 中处理错误的最佳方式是什么？
```

两个 Agent 按顺序回答，后面的 Agent 能看到前面 Agent 的答案。

### 指南 2：跨项目共享 Provider 配置

把 Provider 和 API Key 放在用户级配置中，所有项目共享。也可以用 `krew config init` 交互式完成。

**1. 创建 `~/.krew/settings.toml`：**

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

**2. 各项目的 `.krew/settings.toml` 只定义 Agent：**

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

krew 自动合并两层配置，不需要在每个项目重复 Provider。

### 指南 3：恢复和回退会话

**恢复历史会话：**

```bash
# 命令行启动时恢复
krew --resume

# 运行中恢复
› /resume
```

弹出选择器显示最近的会话，选择一个即可继续。

**回退到某个消息点：**

```
› /rewind
```

弹出选择器显示所有用户消息。选择一个后，该消息之后的内容被丢弃。发送新消息时 krew 会生成新的 session ID（原始会话保持不变）。

### 指南 4：在 CI/CD 中使用 Prompt 模式

在脚本中执行一次性 prompt 并解析输出。

**CI 中的代码审查：**

```bash
git diff HEAD~1 | krew -p "@opus review these changes for bugs" --format json
```

**生成 changelog：**

```bash
git log --oneline v0.5.3..HEAD | krew -p "@gpt summarize these commits as a changelog"
```

**检查 exit code：**

```bash
krew -p "@opus 这段代码有安全问题吗？" --format text
if [ $? -ne 0 ]; then
  echo "Agent 出错" >&2
fi
```

Exit code: `0` = 成功, `1` = Agent 出错, `2` = 参数/配置错误。

### 指南 5：密语私密评估

让一个 Agent 私下评价另一个 Agent 的答案：

```
› @all 提出你们的架构方案
  （两个 Agent 公开回答）

› #opus GPT 的方案有什么缺陷？
  （只有 opus 看得到；其他 Agent 看到占位符）
```

多目标密语组——两个 Agent 私下讨论：

```
› #opus #gemini 讨论一下这两种方案的优劣
  （只有 opus 和 gemini 能看到彼此的回复）
```

### 指南 6：自动放行常用 Shell 命令

不想每次都确认 `ls` 和 `cargo build`？加入白名单：

```toml
[settings]
shell_allow_commands = ["ls", "cat", "cargo build", "cargo test", "git status", "git diff"]
```

匹配是前缀匹配——`cargo build --release` 也会自动放行。

---

## 4. 命令行参数

```
krew [OPTIONS] [COMMAND]

Options:
  -c, --config <PATH>           指定配置文件路径（默认: .krew/settings.toml）
  -a, --agents <NAMES>          仅启用指定 Agent（逗号分隔，如 "gpt,opus"）
      --approval-mode <MODE>    覆盖工具审批策略: suggest | auto-edit | full-auto
      --resume [ID]             恢复历史会话（不指定 ID 则弹出选择器）
  -p, --prompt <PROMPT>         非交互式 prompt 模式（见 §14）
      --format <FORMAT>         -p 模式输出格式: text（默认） | json
  -v, --verbose                 启用 DEBUG 级别日志
  -h, --help                    帮助
  -V, --version                 版本

Commands:
  config init [--user|--project]   交互式配置初始化
  config add <provider|agent>      添加 Provider 或 Agent
  config del <provider|agent>      删除 Provider 或 Agent
  config list <providers|agents>   列出 Provider 或 Agent
  config doctor                    诊断配置问题
  config help                      打印完整配置手册
```

### 示例

```bash
# 默认启动
krew

# 仅使用两个 Agent
krew -a gpt,opus

# 恢复上次会话
krew --resume

# 覆盖审批模式
krew --approval-mode full-auto

# 非交互式 prompt
krew -p "@opus 解释一下 Rust 的所有权机制"

# 交互式配置初始化
krew config init

# 添加新 Provider
krew config add provider

# 诊断配置问题
krew config doctor
```

---

## 5. 配置文件

### 5.1 配置文件位置

krew 采用双层配置系统：

| 文件 | 用途 |
| ---- | ---- |
| `~/.krew/settings.toml` | **用户级**：providers、API keys、偏好设置、全局 MCP |
| `.krew/settings.toml` | **项目级**：Agent 定义、reply_order、项目覆盖 |

两者同时存在时合并。项目级优先：
- 同名 provider：项目级整项替换用户级
- 同名 MCP server：项目级替换
- 标量设置：项目级优先
- `agents` 和 `reply_order` 仅在项目级定义

`--config` 参数可指定项目配置路径，但仍会与用户级配置合并。

### 5.2 全局设置

```toml
[settings]
# 工具审批策略（默认: "suggest"）
#   suggest   — 读操作自动，写/Shell/MCP 需确认
#   auto-edit — 读+写自动，Shell/MCP 需确认
#   full-auto — 全部自动放行
approval_mode = "suggest"

# @all 广播时的回答顺序
reply_order = ["gpt", "opus", "gemini"]

# 自动压缩 token 阈值（默认 120000，0 = 禁用）
auto_compact_threshold = 120000

# 压缩时保留最近 N 轮对话（默认 10）
compact_keep_rounds = 10

# 其他 Agent 消息的 role: "user"（默认）或 "assistant"
# other_agent_role = "user"

# tokio 工作线程数（默认 4）
# worker_threads = 4

# 免审批 shell 命令前缀
# shell_allow_commands = ["ls", "cargo build", "git status"]

# 免审批 fetch_url 域名白名单（支持子域名匹配）
# fetch_allow_domains = ["docs.rs", "github.com"]

# AI-to-AI 路由策略: "immediate"（默认）或 "queued"
# agent_to_agent_routing = "immediate"

# AI-to-AI 最大轮次（默认 10，0 = 禁用）
# agent_to_agent_max_rounds = 10

# Agent 回复语言（不设置 = 不注入语言指令）
# 设置后会在每个 Agent 的 system prompt 中注入：
# "Always respond in {language}. Use {language} for all explanations,
# comments, and communications with the user. Technical terms and code
# identifiers should remain in their original form."
# language = "中文"
```

### 5.3 Agent 定义

```toml
[[agents]]
name = "opus"                    # @ 寻址名（必填，不可重复，不可为 "all"）
display_name = "Claude Opus"     # TUI 显示名（必填）
provider = "anthropic"           # 引用 [providers.*] 条目（必填）
model = "claude-opus-4-6"        # 模型 ID（必填）
color = "magenta"                # 终端颜色: red/green/yellow/blue/magenta/cyan/white
system_prompt = ""               # 自定义系统提示词（可选）
tools = true                     # 启用工具（默认 false）
enable_web_search = false        # 启用原生 Web 搜索（默认 false）
enable_thinking = false          # 显示思考过程（默认 false）
# thinking_effort = "medium"     # 思考力度: low | medium | high

# OpenAI 专用
# api_type = "responses"         # "responses"（Responses API）或 "chat"（Chat Completions）

# 采样参数（均可选，不设置则使用 Provider 默认值）
# sampling.temperature = 0.7     # OpenAI/Google: 0-2, Anthropic: 0-1
# sampling.top_p = 0.9
# sampling.top_k = 40            # 仅 Anthropic/Google
# sampling.max_tokens = 32768    # 默认取模型最大输出值
# sampling.frequency_penalty = 0
# sampling.presence_penalty = 0
# sampling.stop_sequences = ["END"]
```

### 5.4 Provider 配置

每个 Provider 需要 `type` 字段指定类型：`"openai"`、`"anthropic"` 或 `"google"`。

```toml
# OpenAI
[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"       # 环境变量名（推荐）
# api_key = "sk-..."                 # 直接填写（不推荐）
# base_url = "https://api.openai.com"

# Anthropic
[providers.anthropic]
type = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
# base_url = "https://api.anthropic.com"

# Google（API Key 模式）
[providers.google]
type = "google"
api_key_env = "GOOGLE_API_KEY"

# Google（Vertex AI 模式）
[providers.vertex]
type = "google"
vertex_project = "my-project"
vertex_location = "us-central1"

# OpenAI 兼容（如豆包、LiteLLM）
[providers.doubao]
type = "openai"
api_key_env = "DOUBAO_API_KEY"
base_url = "https://ark.cn-beijing.volces.com/api/v3"
```

### 5.5 MCP 服务器配置

```toml
# Stdio 传输（子进程）
[[mcp_servers]]
name = "filesystem"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "."]
env = { SOME_VAR = "$ENV_VAR" }      # 支持 $VAR 展开
trust = "auto"                        # "auto" = 跳过审批, "confirm"（默认）= 按审批策略

# HTTP 传输（Streamable HTTP）
[[mcp_servers]]
name = "remote-tools"
url = "https://mcp.example.com/sse"
headers = { Authorization = "Bearer $TOKEN" }
```

### 5.6 Skill 配置

```toml
[skills]
enabled = true                        # 启用 Skill 系统（默认 true）
# extra_paths = ["/path/to/skills"]   # 额外搜索路径
```

### 5.7 配置校验

krew 启动时自动校验配置，常见错误：
- `reply_order` 引用了不存在的 agent 名称
- Agent 的 `provider` 引用了不存在的 provider
- Agent 名称重复
- 使用了保留名称 `"all"`

---

## 6. 配置管理（`krew config`）

krew 提供一套 `config` 子命令用于交互式配置管理。所有 config 子命令在普通终端模式下运行（不启动 TUI）。

### 6.1 `krew config init`

交互式向导，从零开始设置配置文件。

```bash
krew config init              # 自动检测需要设置什么
krew config init --user       # 仅设置用户级配置（~/.krew/settings.toml）
krew config init --project    # 仅设置项目级配置（.krew/settings.toml）
```

**智能路由**（无标志）：向导检查哪些配置文件已存在，只创建缺少的：
- 两个都不存在 → 先设置用户配置，再设置项目配置
- 用户配置不存在、项目配置存在 → 仅设置用户配置
- 用户配置存在、项目配置不存在 → 仅设置项目配置
- 两个都存在 → 退出并提示

**用户配置设置**引导你添加 Provider：
1. 选择 Provider 类型（Anthropic / OpenAI / Google / OpenAI 兼容）
2. 输入 Provider 名称（根据类型自动建议）
3. 选择 API Key 方式（环境变量或配置文件）
4. 可选设置 base_url（OpenAI 兼容）或 Vertex 字段（Google）
5. 循环 — 继续添加或完成

**项目配置设置**提供两种模式：
- **智能预设**：从你的 Provider 获取可用模型列表，提供单 Agent 或三 Agent 预设，通过模糊搜索选择模型
- **手动设置**：循环式 Agent 创建——选择 Provider、模型、名称、显示名、颜色、Thinking、Web Search；`tools` 默认启用

### 6.2 `krew config add`

向现有配置添加单个 Provider 或 Agent。

```bash
krew config add provider    # 添加 Provider 到用户配置
krew config add agent       # 添加 Agent 到项目配置
```

使用与 `init` 相同的交互式提示。新条目追加到对应配置文件，格式保留写入（注释和格式不受影响）。

### 6.3 `krew config del`

删除 Provider 或 Agent。

```bash
krew config del provider    # 从用户配置删除 Provider
krew config del agent       # 从项目配置删除 Agent
```

显示选择列表，对依赖关系发出警告（如 Agent 引用了该 Provider），确认后删除。

### 6.4 `krew config list`

以表格格式显示 Provider 或 Agent。

```bash
krew config list providers   # 显示所有 Provider 及 Key 状态
krew config list agents      # 显示所有 Agent 及设置
```

**Provider 表格**显示：名称、类型、Key 方式（环境变量 ✅/❌ 或配置文件）、Base URL。

**Agent 表格**显示：名称、显示名、Provider、模型、颜色、Thinking、Web Search。表格下方显示 `reply_order`。

### 6.5 `krew config doctor`

诊断配置的完整性和有效性。

```bash
krew config doctor
```

检查项目：
- 配置文件是否存在、语法是否正确（用户级 + 项目级）
- Provider API Key 是否可用（环境变量是否设置？配置文件中是否有 Key？）
- Agent 的 Provider 引用（引用的 Provider 是否存在？）
- MCP 服务器命令是否可用（命令是否在 PATH 中？）

使用 ✅/❌/⚠️ 指示器。明确报告解析错误（不像运行时加载那样静默回退）。

### 6.6 `krew config help`

将完整配置手册打印到 stdout。

```bash
krew config help
```

涵盖：文件位置、合并规则、所有字段及默认值、CLI 命令参考、示例配置。

---

## 7. 寻址与路由

### @ 寻址

| 语法 | 行为 |
| ---- | ---- |
| `@all <消息>` | 广播给所有 Agent（按 `reply_order` 顺序回答） |
| `@name <消息>` | 仅指定 Agent 回答 |
| `@a @b <消息>` | 多个 Agent 依次回答（按 @ 出现顺序） |
| `<消息>`（无 @） | 发给上一个回答者 |

`@name` 可出现在消息的任意位置。未识别的 `@token` 视为普通文本。消息正文保留完整原文（不剥离 @ token）。

### # 密语（私密消息）

| 语法 | 行为 |
| ---- | ---- |
| `#name <消息>` | 密语给一个 Agent（其他 Agent 看到占位符） |
| `#a #b <消息>` | 密语组（仅组内成员互相可见） |

- `#all` 被禁止（返回错误）
- `@` 和 `#` 不可混用
- Agent 回复自动继承密语目标
- 密语消息在 TUI 中显示锁图标

### AI-to-AI 路由

Agent 回复中 `@mention` 其他 Agent 时自动调度：

- **immediate**（默认）：目标 Agent 插入队列头部
- **queued**：目标 Agent 追加到队列尾部
- 由 `agent_to_agent_routing` 和 `agent_to_agent_max_rounds` 控制
- 密语模式下仅路由组内成员，组外 mention 被忽略

---

## 8. Slash 命令

| 命令 | 描述 |
| ---- | ---- |
| `/clear` | 清屏并开始新会话（别名 `/new`） |
| `/resume` | 列出并恢复历史会话 |
| `/rewind` | 回退到历史消息点（fork 语义） |
| `/agents` | 列出当前 Agent 及 token 用量 |
| `/compact [agent]` | 使用指定 Agent 压缩上下文（默认: reply_order 首个） |
| `/mcp` | 列出 MCP 服务器及工具 |
| `/skills` | 列出可用技能 |
| `/stats` | 显示进程统计（内存、线程） |
| `/help` | 显示所有命令（内置 + 自定义） |
| `/exit` | 退出（别名 `/quit`） |

内置命令优先级高于同名自定义命令。输入 `/` 可弹出补全列表。

---

## 9. 自定义命令

通过 Markdown 文件定义自定义 Slash 命令。

### 发现路径（优先级从高到低）

| 优先级 | 路径 |
| ------ | ---- |
| 1 | `.krew/commands/` |
| 2 | `.agents/commands/` |
| 3 | `.claude/commands/` |
| 4 | `~/.krew/commands/` |
| 5 | `~/.agents/commands/` |
| 6 | `~/.claude/commands/` |

子目录形成命名空间：`commands/review/code.md` → `/review:code`。同名命令 first-found wins。

### 文件格式

```markdown
---
description: 审查代码质量
argument-hint: <文件路径>
---

请审查以下文件的潜在问题: $ARGUMENTS
```

### 参数替换

- `$ARGUMENTS` — 完整参数字符串
- `$1`、`$2`、... — 位置参数（按空格分割），未提供则为空字符串

### Bash 预处理

使用 `` !`command` `` 语法嵌入 Shell 输出：

```markdown
分析这些改动：
!`git diff --cached`
```

命令在会话工作目录执行。失败时替换为错误消息（不中止）。

---

## 10. 工具系统

### 内置工具

| 工具 | 描述 | 审批 |
| ---- | ---- | ---- |
| `read_file` | 读取文件（带行号，支持 offset/limit） | 自动 |
| `glob` | 文件模式匹配 | 自动 |
| `grep` | 内容搜索（正则，支持 include 过滤） | 自动 |
| `write_file` | 创建/覆写文件（自动创建父目录） | 可配 |
| `edit_file` | 搜索替换编辑（唯一匹配验证，diff 预览） | 可配 |
| `shell` | 执行 Shell 命令（超时 120s，输出限制 100KB） | 可配 |
| `fetch_url` | 抓取 URL（HTTP→HTTPS 升级，HTML→Markdown，1MB 限制） | 可配 |
| `activate_skill` | 激活 Skill（有 Skills 时自动注册） | 自动 |

所有文件工具强制路径边界：操作必须在会话工作目录内。

### Shell 工具细节

- **Windows**：使用 Git Bash（搜索顺序：`KREW_BASH_PATH` → PATH 中的 bash.exe（跳过 WSL）→ `C:\Program Files\Git\bin\bash.exe`）
- **Unix**：使用 `KREW_BASH_PATH` → `$SHELL` → `/bin/sh`
- **超时**：`timeout_seconds` 参数（默认 120 秒）
- **输出**：超过 100KB 时截断并标记 `[output truncated at 100KB]`

### fetch_url 细节

- HTTP URL 自动升级为 HTTPS
- 响应大小限制 1MB
- 域名白名单：`fetch_allow_domains`，子域名匹配（如 `docs.github.com` 匹配 `github.com`）
- 超时 30 秒

### 审批策略

| 策略 | 读工具 | 写工具 | Shell | fetch_url | MCP 工具 |
| ---- | ------ | ------ | ----- | --------- | -------- |
| `suggest` | 自动 | 需确认 | 需确认* | 白名单自动，其他确认 | 需确认** |
| `auto-edit` | 自动 | 自动 | 需确认* | 白名单自动，其他确认 | 需确认** |
| `full-auto` | 自动 | 自动 | 自动 | 自动 | 自动 |

\* 匹配 `shell_allow_commands` 前缀的命令自动放行。
\** MCP 工具 `trust = "auto"` 跳过审批；`trust = "confirm"` 参考 annotations。

**审批快捷键**：`y` 批准 / `a` 会话级批准 / `n` 或 `Esc` 拒绝 / `Ctrl+C` 中止

**会话缓存**："会话级批准"按命令前缀（shell）、域名（fetch_url）或工具名（其他）缓存。

---

## 11. MCP 集成

krew 支持通过 [Model Context Protocol](https://modelcontextprotocol.io) 服务器扩展 Agent 能力。

### 配置

见 [§5.5](#55-mcp-服务器配置)。或运行 `krew config help` 获取完整参考。

### 工具命名

MCP 工具使用限定名：`mcp__<server>__<tool>`（内部）/ `mcp:<server>/<tool>`（显示）。

### 信任级别

| 信任 | 行为 |
| ---- | ---- |
| `auto` | 该服务器所有工具跳过审批 |
| `confirm`（默认） | 按 `approval_mode` 规则，参考工具 annotations |

MCP 服务器在会话启动时初始化。使用 `/mcp` 查看已连接的服务器和工具。

---

## 12. Skill 系统

Skill 为特定任务提供专业指令。

### Skill 目录结构

```
my-skill/
├── SKILL.md            # 必需：skill 定义
├── scripts/            # 可选：辅助脚本
├── references/         # 可选：参考资料
└── assets/             # 可选：其他资源
```

### SKILL.md 格式

```markdown
---
name: code-review
description: 遵循最佳实践进行代码审查
---

## 指令
审查代码时，检查以下方面...
```

必需字段：`name`、`description`。可选字段：`compatibility`、`metadata`。

### 发现路径

| 优先级 | 路径 |
| ------ | ---- |
| 1 | `.krew/skills/` |
| 2 | `.agents/skills/` |
| 3 | `.claude/skills/` |
| 4 | `~/.krew/skills/` |
| 5 | `~/.agents/skills/` |
| 6 | `~/.claude/skills/` |
| 7 | `skills.extra_paths` 中的路径 |

扫描深度：4 层。跳过：`.git/`、`node_modules/`、`target/`。同名 Skill：first-found wins。

### 工作原理

1. 启动时发现所有 Skill，构建 Catalog
2. Catalog 注入到每个 Agent 的系统提示词
3. Agent 遇到匹配任务时调用 `activate_skill` 加载完整指令
4. 返回 SKILL.md 正文、Skill 目录路径和资源文件列表

使用 `/skills` 查看可用技能。

---

## 13. 会话管理

### 持久化

每条消息实时保存到 `.krew/sessions/<session_id>.toml`。即使 krew 异常退出，对话也不会丢失。

### 恢复会话

```bash
# 启动时恢复
krew --resume

# 运行中恢复
/resume
```

### 回退（Rewind）

`/rewind` 让你回到任意用户消息点并开始新分支：

1. 弹出选择器，列出所有用户消息
2. 选择回退点
3. 该点之后的消息被丢弃
4. 发送新消息时生成新 session ID
5. 原始会话保持不变

### 自动压缩

当 `prompt_tokens` 超过 `auto_compact_threshold` 时，krew 在下一次消息前自动压缩历史：

- 使用 `reply_order` 中第一个 Agent 执行压缩
- 保留最近 `compact_keep_rounds` 轮（默认 10）
- 密语消息从压缩区提取并保留（不参与压缩）
- 压缩前自动创建备份

### Token 追踪

使用 `/agents` 查看各 Agent 的 token 用量（输入/输出/总计）。

---

## 14. Prompt 模式

非交互模式，适用于脚本和 CI/CD。

### 基本用法

```bash
krew -p "@opus 解释一下 Rust 的所有权机制"
```

### 要求

- 必须包含至少一个 `@agent`、`@all` 或 `#agent` 寻址
- `#all` 被拒绝
- 无 @ 或 # → exit code 2

### stdin 管道

```bash
cat src/main.rs | krew -p "@opus review this"
git diff | krew -p "@gpt 总结一下这些改动"
```

stdin 内容以 `<stdin>...</stdin>` 标签包裹后拼接到 prompt 前。寻址仅从 `-p` 参数解析，stdin 中的 `@agent` 不影响路由。

### 输出格式

**Text**（默认，`--format text`）：流式输出。

```
[opus]
⚡ read_file(src/main.rs)
   ⎿  done

这段代码有以下问题...
```

**JSON**（`--format json`）：JSONL，每行一个事件。

```json
{"agent":"opus","type":"tool_start","tool":"read_file","arguments":"{\"file_path\":\"src/main.rs\"}"}
{"agent":"opus","type":"tool_output","text":"..."}
{"agent":"opus","type":"tool_done","tool":"read_file","summary":"done"}
{"agent":"opus","type":"text","content":"这段代码有以下问题..."}
```

密语模式下 JSON 包含 `"whisper_targets": [...]` 字段。Text 格式 header 显示 `[agent] [whisper]`。

### 完整 JSON 事件参考

| `type` | 字段 | 说明 |
| ------ | ---- | ---- |
| `text` | `agent`, `content` | Agent 文本回复（流式结束后一次输出） |
| `tool_start` | `agent`, `tool`, `arguments` | 工具调用开始 |
| `tool_output` | `agent`, `text` | 工具实时输出（如 shell 流式输出） |
| `tool_done` | `agent`, `tool`, `summary` | 工具调用完成 |
| `server_tool_start` | `agent`, `tool` | 服务端工具开始（如 Web Search） |
| `server_tool_done` | `agent`, `tool`, `query` | 服务端工具完成 |

所有事件包含 `"agent"` 字段。密语事件额外包含 `"whisper_targets": [...]`。

**示例：完整 JSON 会话**

```json
{"agent":"opus","type":"server_tool_start","tool":"web_search"}
{"agent":"opus","type":"server_tool_done","tool":"web_search","query":"rust error handling"}
{"agent":"opus","type":"tool_start","tool":"read_file","arguments":"{\"file_path\":\"src/main.rs\"}"}
{"agent":"opus","type":"tool_done","tool":"read_file","summary":"1,234 bytes"}
{"agent":"opus","type":"tool_start","tool":"shell","arguments":"{\"command\":\"cargo test\"}"}
{"agent":"opus","type":"tool_output","text":"running 42 tests\n"}
{"agent":"opus","type":"tool_output","text":"test result: ok. 42 passed\n"}
{"agent":"opus","type":"tool_done","tool":"shell","summary":"exit 0"}
{"agent":"opus","type":"text","content":"所有测试通过。以下是我的分析..."}
```

### Exit Code

| 代码 | 含义 |
| ---- | ---- |
| 0 | 所有 Agent 成功完成 |
| 1 | 有 Agent 出错（API 错误等） |
| 2 | 参数/配置错误（缺少寻址、参数冲突等） |

### 限制

- `-p` 与 `--resume` 互斥
- 所有工具以 `full-auto` 模式运行（无审批提示）
- 支持 AI-to-AI 路由，受 `agent_to_agent_max_rounds` 限制

---

## 15. 项目指令 (AGENTS.md)

在项目目录放置 `AGENTS.md` 文件，为所有 Agent 提供项目上下文（架构说明、编码规范等）。

### 发现规则

krew 从工作目录向上遍历到文件系统根目录，收集所有找到的 `AGENTS.md` 文件。合并顺序：祖先在前、子目录在后（子目录内容可补充或覆盖祖先的通用指令）。

### 注入方式

内容以 `<project-instructions>` 标签包裹，注入到每个 Agent 的系统提示词中（在 Skill Catalog 和 Agent 自身 `system_prompt` 之前）。

### 限制

- 单文件最大 100KB（超出截断并附带警告）
- 非 UTF-8 文件跳过（记录 warning 日志）

---

## 16. 文件路径与加载优先级

### 配置文件

```
优先级（高到低）：
  CLI 参数 (--approval-mode, --agents 等)
    ↓ 覆盖
  .krew/settings.toml          （项目级配置）
    ↓ 覆盖
  ~/.krew/settings.toml         （用户级配置）
    ↓ 覆盖
  内置默认值
```

### 数据目录

| 路径 | 内容 |
| ---- | ---- |
| `.krew/settings.toml` | 项目配置 |
| `.krew/sessions/` | 会话 TOML 文件 |
| `.krew/history` | 输入历史（跨 session 保留） |
| `.krew/logs/` | 日志文件（按天滚动，保留 7 天） |
| `~/.krew/settings.toml` | 用户配置 |

### 命令发现路径（优先级从高到低）

| # | 路径 | 范围 |
| - | ---- | ---- |
| 1 | `.krew/commands/` | 项目级，krew 专属 |
| 2 | `.agents/commands/` | 项目级，跨客户端 |
| 3 | `.claude/commands/` | 项目级，Claude Code 兼容 |
| 4 | `~/.krew/commands/` | 用户级，krew 专属 |
| 5 | `~/.agents/commands/` | 用户级，跨客户端 |
| 6 | `~/.claude/commands/` | 用户级，Claude Code 兼容 |

### Skill 发现路径（优先级从高到低）

| # | 路径 | 范围 |
| - | ---- | ---- |
| 1 | `.krew/skills/` | 项目级，krew 专属 |
| 2 | `.agents/skills/` | 项目级，跨客户端 |
| 3 | `.claude/skills/` | 项目级，Claude Code 兼容 |
| 4 | `~/.krew/skills/` | 用户级，krew 专属 |
| 5 | `~/.agents/skills/` | 用户级，跨客户端 |
| 6 | `~/.claude/skills/` | 用户级，Claude Code 兼容 |
| 7 | `skills.extra_paths` 条目 | 配置指定 |

### 项目指令发现

```
AGENTS.md 加载顺序：
  / （文件系统根）         ← 最先合并
    ↓
  /path/to/               ← 祖先目录
    ↓
  /path/to/project/       ← 工作目录（最后合并，优先级最高）
```

所有发现均采用 **first-found wins** 策略处理同名条目。

---

## 17. 快捷键

### 对话模式

| 快捷键 | 操作 |
| ------ | ---- |
| `Enter` | 发送消息 |
| `Shift+Enter` / `Ctrl+J` | 换行 |
| `↑` / `↓` | 浏览输入历史 |
| `@` | 打开 Agent 补全弹窗（含 "all"） |
| `#` | 打开密语目标弹窗（不含 "all"） |
| `/` | 打开 Slash 命令弹窗 |
| `Esc` | 取消当前 Agent 流式输出 |
| `Ctrl+C`（连按两次） | 退出程序 |

### 补全弹窗

| 快捷键 | 操作 |
| ------ | ---- |
| `↑` / `↓` | 导航 |
| `Tab` / `Enter` | 确认选择 |
| `Esc` | 关闭弹窗 |

### 审批浮层

| 快捷键 | 操作 |
| ------ | ---- |
| `y` | 本次批准 |
| `a` | 会话级批准（同工具+上下文） |
| `n` / `Esc` | 拒绝 |
| `Enter` | 确认当前选项 |
| `↑` / `↓` | 导航选项 |
| `Ctrl+C` | 中止整个 Agent 回合 |

---

## 18. 常见问题

### Windows 上提示 "Git Bash not found"

krew 在 Windows 上使用 Git Bash 执行 Shell 命令。请安装 [Git for Windows](https://git-scm.com/download/win) 或设置环境变量：

```powershell
$env:KREW_BASH_PATH = "C:\Program Files\Git\bin\bash.exe"
```

### API Key 错误（401/403）

确认环境变量已设置：

```bash
# macOS / Linux
echo $OPENAI_API_KEY

# Windows PowerShell
echo $env:OPENAI_API_KEY
```

如果变量已设置但仍然报认证错误，检查 Key 是否过期以及权限是否正确。

### 为什么 `#all` 报错？

`#all`（对所有人密语）被故意禁止——对所有 Agent 密语在语义上等同于普通消息，所以不允许使用。请改用 `@all` 进行广播。

### 为什么消息没有发给任何 Agent？

如果你输入消息时没有 `@` 或 `#` 前缀，且之前没有 Agent 回答过（比如会话刚开始），krew 不知道发给谁。请用 `@name` 或 `@all` 指定目标。

### 为什么 Shell 一直要求确认？

Shell 命令在 `suggest` 和 `auto-edit` 模式下默认需要确认。要自动放行常用命令，加入白名单：

```toml
[settings]
shell_allow_commands = ["ls", "cargo", "git status", "git diff"]
```

匹配是**前缀匹配**：`"cargo"` 会自动放行 `cargo build`、`cargo test` 等。要跳过所有审批，使用 `--approval-mode full-auto`（请谨慎使用）。

按下 `a`（会话级批准）后，同一命令前缀在当前会话中不再询问。

### 配置校验错误

运行 `krew config doctor` 进行全面诊断，或使用 `--verbose` 查看详细错误：

```bash
krew config doctor
krew --verbose
```

常见原因：
- `reply_order` 中的 Agent 名称没有在 `[[agents]]` 中定义
- Agent 的 `provider` 没有匹配的 `[providers.*]` 条目
- 两个 Agent 使用了相同的 `name`
- Agent 名称使用了保留字 `"all"`

### Token 超限 / 上下文长度错误

如果 LLM 返回上下文长度错误，说明对话太长了。解决方案：

1. **手动压缩**：`/compact`（或 `/compact opus` 指定 Agent）
2. **降低阈值**：设置 `auto_compact_threshold = 80000` 让自动压缩更早触发
3. **重新开始**：`/clear` 开始新会话

### 日志文件在哪里？

日志写在项目目录的 `.krew/logs/` 下。文件按天滚动，7 天后自动清理。使用 `--verbose` 启用 DEBUG 级别的详细日志。

### 自定义命令没有出现

检查以下几点：
1. `.md` 文件是否在[发现路径](#16-文件路径与加载优先级)中（如 `.krew/commands/my-command.md`）
2. 文件是否有合法的 YAML frontmatter（没有 frontmatter 也行——它是可选的）
3. 是否存在同名的内置命令（内置命令优先级更高）
4. 添加新命令文件后需要重启 krew

### Agent 不使用工具

确认 Agent 配置中设置了 `tools = true`。同时检查 Agent 的 Provider 是否支持 tool use（所有内置 Provider 都支持）。
