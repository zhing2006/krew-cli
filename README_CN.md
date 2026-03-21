<div align="center">

# krew-cli

**命令行多 AI Agent 协作会话工具**

在一个终端中同时与多个 AI 模型（GPT、Claude、Gemini 等）对话 —— 像组织一场 AI 圆桌会议。

[![CI](https://github.com/ZHing2006/krew-cli/actions/workflows/release.yml/badge.svg)](https://github.com/ZHing2006/krew-cli/actions/workflows/release.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![npm](https://img.shields.io/npm/v/@zhing2026/krew)](https://www.npmjs.com/package/@zhing2026/krew)

[English](README.md) | [中文](README_CN.md)

</div>

---

## 功能特性

- **多 Agent 会话** — 在一个终端中同时与多个 AI 模型对话
- **@ 寻址** — `@all` 广播，`@name` 指定，无前缀自动发给上一个回答者
- **# 密语** — `#name` 发送私密消息，其他 Agent 不可见；支持多目标密语组
- **共享上下文** — 普通消息全员共享，密语消息按可见性过滤，确保讨论全貌与隐私兼顾
- **AI 间路由** — Agent 可以 `@mention` 其他 Agent，自动调度（支持 immediate/queued 策略）
- **内置工具** — 文件读写编辑、Shell 执行、glob/grep 搜索、URL 抓取、Skill 激活
- **MCP 集成** — 通过 Model Context Protocol 服务器扩展 Agent 能力（stdio + HTTP）
- **Skill 系统** — 可发现、可激活的技能，通过 `SKILL.md` 定义专业指令
- **自定义命令** — 通过 Markdown 文件定义自定义 Slash 命令，支持参数替换和 Bash 预处理
- **会话持久化** — 随时保存和恢复对话；`/rewind` 可从任意历史点分叉
- **Token 追踪与自动压缩** — 实时 token 用量显示；超过阈值自动压缩上下文，保留密语消息
- **Prompt 模式（`-p`）** — 非交互模式，适用于脚本和 CI/CD，支持 text/JSON 输出和 stdin 管道
- **流式输出** — ~60Hz 逐 token 渲染，支持 Markdown、语法高亮和 Agent 颜色区分
- **思考/推理** — 显示模型思考过程（可配置力度：low/medium/high）
- **Web 搜索** — Provider 原生 Web 搜索（OpenAI Responses、OpenAI Chat、Anthropic、Gemini）
- **采样参数** — 每个 Agent 可独立配置 temperature、top_p、max_tokens 等
- **项目指令** — `AGENTS.md` 文件自动注入系统提示词（支持层级化加载）

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
| Azure OpenAI | GPT-5.2 (Azure) | Responses / Chat Completions |
| Anthropic | Claude Opus 4.6, Sonnet 4.6 | Messages |
| Google | Gemini 3.1 Pro | generateContent（+ Vertex AI） |
| OpenAI 兼容 | 豆包、LiteLLM 等 | Responses / Chat Completions |

## 快速开始

### 1. 创建配置文件

在项目目录下创建 `.krew/settings.toml`：

```toml
[settings]
approval_mode = "suggest"
reply_order = ["gpt", "opus"]
auto_compact_threshold = 120000

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
api_key_env = "OPENAI_API_KEY"

[providers.anthropic]
api_key_env = "ANTHROPIC_API_KEY"
```

### 2. 设置 API Key

```bash
export OPENAI_API_KEY="sk-..."
export ANTHROPIC_API_KEY="sk-ant-..."
```

### 3. 启动

```bash
krew
```

## 使用方式

```
krew [OPTIONS]

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
| `suggest` | 自动 | 需确认 | 需确认* | 白名单自动 | 需确认 |
| `auto-edit` | 自动 | 自动 | 需确认* | 白名单自动 | 需确认 |
| `full-auto` | 自动 | 自动 | 自动 | 自动 | 自动 |

\* Shell 命令匹配 `shell_allow_commands` 前缀时自动放行。

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
