## Why

krew 是一个多 AI Agent 协作 CLI 工具，Agent 可以调用工具（读写文件、Shell 等）来帮用户完成实际任务。但目前 Agent 的系统提示词只能通过 `.krew/settings.toml` 中的 `system_prompt` 字段静态配置，没有机制让 Agent 自动了解当前项目的架构、约定和编码规范。

主流 AI 编码工具都支持项目级指令文件：Claude Code 读取 `CLAUDE.md`、OpenAI Codex 读取 `AGENTS.md`、GitHub Copilot 读取 `.github/copilot-instructions.md`。krew 作为多 Agent 协作工具，应当支持类似的机制，让用户在项目中放置指令文件，所有 Agent 启动时自动加载。

## What Changes

- 新增项目级指令文件加载机制：krew 启动时自动扫描工作目录下的指令文件（如 `AGENTS.md`），读取内容并注入到所有 Agent 的系统提示词中
- 支持层级化指令文件：工作目录及其父目录中的指令文件均会被加载，子目录的内容优先级更高（类似 `.gitignore` 的层级机制）
- 更新 PDD 文档：在配置系统章节新增指令文件说明
- 更新 TDD 文档：在配置管理模块新增指令文件加载的技术设计

## Capabilities

### New Capabilities
- `project-instructions`: 项目级指令文件（AGENTS.md）的发现、加载和注入机制

### Modified Capabilities
- `config-types`: 配置数据结构中需要新增指令文件相关的运行时字段（加载后的指令内容）

## Impact

- **代码影响**：主要影响 `krew-config`（指令文件发现与加载）和 `krew-core`（系统提示词构建时注入指令内容）
- **文档影响**：需更新 PDD §4.6 配置系统 和 TDD §3.7 配置管理
- **用户影响**：用户可在项目根目录创建 `AGENTS.md` 文件来为所有 Agent 提供项目上下文，无需修改 settings.toml
