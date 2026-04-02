## Why

Agent Memory 系统已经实现了记忆的持久化存储，但随着使用时间增长，记忆文件会累积大量冗余、过时、甚至相互矛盾的条目。用户需要一个主动整理机制来保持记忆质量 —— 合并重复条目、删除陈旧事实、修剪臃肿索引。`/dream` 命令让用户指定一个 Agent 执行"做梦"式的记忆整理，类似 free-code 的 Memory Consolidation 功能，但适配 krew 的多 Agent 两层记忆架构。

## What Changes

- 新增 `/dream` slash 命令，支持 `global|agent|all` scope 和 `@agent` 寻址
- 构建 consolidation prompt（3 阶段：Orient → Consolidate → Prune），根据 scope 动态注入目标目录
- `/dream` 以普通用户消息方式注入当前会话，Agent 使用现有 tools（read_file, write_file, edit_file, glob, grep）执行整理
- `agent` scope 支持 `@all`（每个 Agent 串行整理各自私有目录），`global` 和 `all` scope 仅支持单 Agent

## Capabilities

### New Capabilities
- `dream-command`: `/dream` slash 命令解析、参数校验、scope 路由和 consolidation prompt 构建

### Modified Capabilities
- `slash-commands`: 新增 `/dream` 命令注册（解析、帮助列表、tab 补全）

## Impact

- `crates/krew-core/src/command.rs` — 新增 `SlashCommand::Dream` 变体
- `crates/krew-core/src/dream.rs` — 新模块：consolidation prompt 构建
- `crates/krew-core/src/lib.rs` — 注册 `dream` 模块
- `crates/krew-cli/` — TUI 层处理 `/dream` 命令的执行流程（消息注入 + Agent dispatch）
