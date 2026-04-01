<div align="center">

# krew-cli

**命令行多 AI Agent 协作会话工具**

在一个终端中同时与多个 AI 模型（GPT、Claude、Gemini 等）对话 —— 像组织一场 AI 圆桌会议。

[![CI](https://github.com/ZHing2006/krew-cli/actions/workflows/release.yml/badge.svg)](https://github.com/ZHing2006/krew-cli/actions/workflows/release.yml)
[![npm](https://img.shields.io/npm/v/@zhing2026/krew)](https://www.npmjs.com/package/@zhing2026/krew)
[![npm downloads](https://img.shields.io/npm/dm/@zhing2026/krew)](https://www.npmjs.com/package/@zhing2026/krew)
[![GitHub stars](https://img.shields.io/github/stars/ZHing2006/krew-cli)](https://github.com/ZHing2006/krew-cli/stargazers)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/built%20with-Rust-dea584)](https://www.rust-lang.org/)

[English](README.md) | [中文](README_CN.md)

</div>

<p align="center">
  <img src="docs/images/demo_cn.gif" alt="krew-cli 演示" width="800">
</p>

---

## 功能特性

- **多 Agent 会话** — 在一个终端中同时与多个 AI 模型对话
- **@ 寻址** — `@all` 广播，`@name` 指定，无前缀自动发给上一个回答者
- **# 密语** — `#name` 发送私密消息，其他 Agent 不可见；支持多目标密语组
- **共享上下文** — 普通消息全员共享，密语消息按可见性过滤，确保讨论全貌与隐私兼顾
- **AI 间路由** — Agent 可以 `@mention` 其他 Agent，自动调度（支持 immediate/queued 策略）
- **内置工具** — 文件读写编辑、图片查看、Shell 执行、glob/grep 搜索、URL 抓取、Skill 激活
- **MCP 集成** — 通过 Model Context Protocol 服务器扩展 Agent 能力（stdio + HTTP）
- **Skill 系统** — 可发现、可激活的技能，通过 `SKILL.md` 定义专业指令
- **自定义命令** — 通过 Markdown 文件定义自定义 Slash 命令，支持参数替换和 Bash 预处理
- **配置向导** — 交互式 `krew config init` 配置、`config doctor` 诊断、Provider/Agent 增删管理
- **会话持久化** — 随时保存和恢复对话；`/rewind` 可从任意历史点分叉
- **Token 追踪与自动压缩** — 实时 token 用量显示；超过阈值自动压缩上下文，保留密语消息
- **Prompt 模式（`-p`）** — 非交互模式，适用于脚本和 CI/CD，支持 text/JSON 输出和 stdin 管道
- **流式输出** — ~60Hz 逐 token 渲染，支持 Markdown、语法高亮和 Agent 颜色区分
- **思考/推理** — 显示模型思考过程（可配置力度：low/medium/high）
- **Web 搜索** — Provider 原生 Web 搜索（OpenAI Responses、OpenAI Chat、Anthropic、Gemini）
- **采样参数** — 每个 Agent 可独立配置 temperature、top_p、max_tokens 等
- **项目指令** — `AGENTS.md` 文件自动注入系统提示词（支持层级化加载）
- **Sub-Agent（实验性）** — 将专项任务委派给隔离上下文的子代理执行，避免 tool call 污染主对话；兼容 `.claude/agents/*.md` 定义格式（默认禁用，需在 `.krew/settings.toml` 中设置 `sub_agent_enabled = true` 启用）

## 安装

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

```bash
cargo install --path crates/krew-cli
```

## 支持的 Provider

| Provider | 模型（示例） | API |
| -------- | ------------ | --- |
| OpenAI | GPT-5.2 | Responses / Chat Completions |
| Anthropic | Claude Opus 4.6, Sonnet 4.6 | Messages |
| Google | Gemini 3.1 Pro | generateContent（+ Vertex AI） |
| OpenAI 兼容 | 豆包、LiteLLM 等 | Responses / Chat Completions |

## 快速开始

### 方式 A：交互式配置（推荐）

**1. 安装**

```bash
npm install -g @zhing2026/krew
```

**2. 运行配置向导**

```bash
krew config init
```

向导会一步一步引导你完成配置：

```
=== User Configuration (Providers) ===

Add provider [1]
Select provider type:
> Anthropic
  OpenAI
  Google
  OpenAI-Compatible
Provider name [anthropic]: ↵
API key storage method:
> Environment variable
  Store in config file
Environment variable name [ANTHROPIC_API_KEY]: ↵
Base URL [https://api.anthropic.com]: ↵
Added provider "anthropic" (Anthropic)
Add another provider? (y/N): N

=== Project Configuration (Agents) ===

Select setup mode:
> Smart Preset
  Manual Setup
Fetching available models...
Select preset:
> Single Agent
  Three Agents
Select model: claude-sonnet-4-6 (anthropic)
Enable thinking for claude? (Y/n): Y
Write this configuration? (Y/n): Y
```

> **提示：** 你可以选择 "Store in config file" 而不是 "Environment variable"——向导会直接提示你输入 API Key 并保存到配置文件中，无需设置环境变量。

**3. 启动**

```bash
krew
```

### 方式 B：手动创建配置文件

```bash
npm install -g @zhing2026/krew
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

创建 `~/.krew/settings.toml`（用户级——跨项目共享）：

```toml
[providers.openai]
type = "openai"
api_key_env = "OPENAI_API_KEY"

[providers.anthropic]
type = "anthropic"
api_key_env = "ANTHROPIC_API_KEY"
```

在项目目录下创建 `.krew/settings.toml`（项目级——Agent 定义）：

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
```

```bash
krew
```

## 使用方式

```
krew [OPTIONS] [COMMAND]

Options:
  -c, --config <PATH>           指定配置文件路径
  -a, --agents <NAMES>          启用的 Agent（逗号分隔）
      --approval-mode <MODE>    工具审批策略: suggest | auto-edit | full-auto
      --resume [ID]             恢复指定会话
  -p, --prompt <PROMPT>         非交互式 prompt 模式
      --format <FORMAT>         -p 模式输出格式: text | json
  -v, --verbose                 详细输出
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

### 寻址语法

```
› @all 用 Rust 实现一个高性能的消息队列，应该选择什么数据结构？
› @opus 你提到的无锁环形缓冲区，能展开讲讲吗？
› 继续说                          # 发给上一个回答者
› #opus 你觉得 GPT 的方案有什么问题？   # 密语（私密消息）
```

### Slash 命令

| 命令 | 描述 |
| ---- | ---- |
| `/clear` | 清屏并开始新会话（别名 `/new`） |
| `/resume` | 恢复历史会话 |
| `/rewind` | 回退到历史消息，从该点分叉对话 |
| `/agents` | 列出当前 Agent 及 token 用量 |
| `/compact [agent]` | 使用指定 Agent 压缩上下文 |
| `/mcp` | 列出 MCP 服务器及工具 |
| `/skills` | 列出可用技能 |
| `/tools` | 按 Agent 列出可用工具 |
| `/stats` | 显示进程统计（内存、线程） |
| `/help` | 显示所有命令（含自定义命令） |
| `/exit` | 退出（别名 `/quit`） |

### Prompt 模式

```bash
# 单 Agent
krew -p "@opus 解释一下 Rust 的所有权机制"

# 管道输入
cat src/main.rs | krew -p "@opus review this code"

# JSON 格式输出
krew -p "@all hello" --format json

# 密语模式
krew -p "#opus 你觉得 GPT 的方案怎么样？"
```

### 工具审批策略

| 策略 | 读操作 | 写操作 | Shell | fetch_url | MCP |
| ---- | ------ | ------ | ----- | --------- | --- |
| `suggest` | 自动 | 需确认 | 需确认 | 需确认 | 需确认 |
| `auto-edit` | 自动 | 自动 | 需确认 | 需确认 | 需确认 |
| `full-auto` | 自动 | 自动 | 自动 | 自动 | 自动 |

通过 `[[allow_rules]]`、`[[deny_rules]]`、`[[ask_rules]]` 配置细粒度权限控制。保护路径（`.git/`、`.env` 等）始终受保护。

## 架构

Rust · Tokio · Ratatui — 6 crate 工作空间，5 平台静态链接二进制。

```
krew-cli          CLI 入口 + TUI（ratatui, crossterm）
  └── krew-core   会话管理、Agent Loop、路由、Slash 命令、Skill、自定义命令
        ├── krew-llm      LLM Provider 抽象（OpenAI/Anthropic/Google/Compatible）
        ├── krew-tools    工具 trait + 内置工具（8 个）+ MCP 客户端（rmcp）
        ├── krew-storage  TOML 会话持久化 + 输入历史
        └── krew-config   TOML 配置加载 + AGENTS.md 指令
```

**文档：**
- [使用手册](docs/MANUAL_CN.md)（[English](docs/MANUAL.md)）— 安装、配置、使用指南
- [PDD](docs/PDD.md) — 产品设计
- [TDD](docs/TDD.md) — 技术设计

## 许可证

[Apache-2.0](LICENSE)
