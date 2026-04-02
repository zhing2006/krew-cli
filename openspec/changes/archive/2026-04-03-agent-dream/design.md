## Context

Agent Memory 系统已实现两层持久化存储（Global `.krew/memory/` + Per-Agent `.krew/memory/agents/{name}/`），Agent 通过 prompt 指令使用 `read_file`/`write_file` 工具管理记忆文件。随着使用时间增长，记忆文件会累积冗余、过时、矛盾的条目。需要一个主动整理机制来保持记忆质量。

free-code 的 Dream（Memory Consolidation）系统已验证了这一模式：让 LLM 执行"做梦"式的反思，整理和修剪记忆文件。krew 需要适配多 Agent 和两层存储架构。

## Goals / Non-Goals

**Goals:**
- 提供 `/dream` slash 命令，用户手动触发记忆整理
- 支持 `global`/`agent`/`all` 三种 scope，控制整理边界
- 支持 `@agent` 寻址指定执行者（不支持 `@all`）
- 以 whisper 消息方式注入，确保 dream 过程对其他 Agent 不可见
- 复用现有 Agent Loop 和工具系统
- 构建适配两层存储的 consolidation prompt（Orient → Consolidate → Prune）
- Dream 执行期间通过 `exclude_tools` 收窄工具集，排除 shell 等非文件工具

**Non-Goals:**
- 不实现自动触发（时间门控、会话计数门控）—— 仅手动触发
- 不扫描 session transcripts —— 记忆已包含会话中的重要信息
- 不引入新的 tool 或子进程机制 —— 使用现有 tools
- 不实现进度追踪 UI（如 free-code 的 DreamTask footer pill）

## Decisions

### Decision 1: 消息注入方式 —— 作为 whisper message

Dream prompt 作为一条 **whisper message** 注入当前会话历史，whisper_targets 设为目标 agent，确保 dream 的过程（包括 tool call/result）对其他 Agent 完全不可见。

这样做的核心原因是 **agent scope 的记忆隐私**：dream 执行中 agent 会读取私有 memory 文件（`.krew/memory/agents/{name}/`），如果用普通消息注入，`prepare_messages_for_agent()` 会将 tool call/result 折叠为文本，后续 agent 就能看到私有记忆内容。Whisper 的可见性过滤（`apply_whisper_filter()`）能彻底隔离这些内容。

**替代方案 1：** 普通 user message —— 简单但会泄露 agent scope 的私有记忆，与 Personal memory "Private to you" 的定义冲突。
**替代方案 2：** 独立上下文（forked agent / 临时 loop）—— 隔离最强但需要新执行路径，复杂度高。
**替代方案 3：** 普通 user message + whisper 仅用于 agent scope —— 不一致，增加实现分支。

**Rationale:** Whisper 复用现有基础设施，隔离性足够，三个 scope 统一使用同一注入方式，规则简单一致。用户仍可在 TUI 中看到 dream 过程（whisper 对用户可见）。

**注意：** Whisper 消息不会被 `/compact` 压缩（compact 会跳过 whisper 并原样保留），因此 dream 历史会长期留在上下文中。这是隐私隔离的必要代价。如果上下文过长，用户可以开新 session。

### Decision 2: 命令格式和路由

```
/dream <scope> @<agent>
```

| 格式 | 行为 |
|------|------|
| `/dream global @opus` | opus 整理 `.krew/memory/`（Global MEMORY.md + topic files） |
| `/dream agent @opus` | opus 整理 `.krew/memory/agents/opus/` |
| `/dream all @opus` | opus 整理 Global + 自己的 agents/opus/ 目录 |
| `/dream` | 显示用法提示 |

约束：
- 所有 scope 均不支持 `@all` —— 每次只允许指定单个 Agent
- `@agent` 必须是已配置且可用的 Agent
- Agent 必须启用 tools（`tools = true`）—— 否则无法读写文件

**Rationale:** 不支持 `@all` 是为了配合 whisper 隔离。Whisper 的可见性以 whisper group 为边界，如果 `@all` 串行执行多个 agent 的 dream，同一 whisper group 内的 agent 仍能互相看到对方的 dream 过程（包括私有记忆内容）。逐个执行 `/dream agent @opus`、`/dream agent @sonnet` 则每次创建独立的 whisper，天然隔离。Dream 不是高频操作，手动逐个触发完全可接受。

**替代方案：** 无 scope 参数（总是整理全部）—— 但两层存储需要精细控制，全局记忆整理权应明确授予。

### Decision 3: Consolidation Prompt —— 3 阶段结构

基于 free-code 的 `buildConsolidationPrompt`，去掉 session transcript 扫描，适配两层存储：

```
Phase 1 — Orient: glob 目录 + 读 MEMORY.md + 浏览 topic 文件
Phase 2 — Consolidate: 合并重复、删除陈旧、修复矛盾、转换日期
Phase 3 — Prune index: 保持 MEMORY.md < 200 行 / 25KB
```

Prompt 根据 scope 动态注入目标目录：
- `global`: 只包含 `.krew/memory/` 部分
- `agent`: 只包含 `.krew/memory/agents/{name}/` 部分
- `all`: 两者都包含，明确指出两个 MEMORY.md 是独立索引

### Decision 4: 实现位置

- `SlashCommand::Dream` 变体 → `crates/krew-core/src/command.rs`
- `dream` 模块（prompt 构建 + scope/agent 解析）→ `crates/krew-core/src/dream.rs`
- TUI 层执行逻辑（参数校验、消息注入、agent dispatch）→ `crates/krew-cli/src/app/commands.rs`
- 复用 `pending_agents` 队列 dispatch 机制，和普通消息发送路径一致

### Decision 5: 工具集收窄 —— 通过 exclude_tools 限制

Dream 执行时通过 `start_completion()` 的 `exclude_tools` 参数排除不需要的 function tools：

- **排除**：`shell`、`fetch_url`、`activate_skill`、`run_agent`
- **保留**：`read_file`、`write_file`、`edit_file`、`glob`、`grep`
- **不限制**：provider-native 的 `web_search`（如果 agent 配置了 `enable_web_search = true`）—— 联网搜索对验证记忆中的事实是否过时有帮助

Scope 的目录边界由 prompt 引导即可 —— agent 在正常对话中本来就能读写两层 memory，dream 时写错层最多是整理效果不好，不是安全问题。不需要改动 `AgentLoopContext`、`ApprovalContext` 或 `load_memory_prompt()` 的签名。

**Rationale:** `shell` 能绕过 approval 直接操作文件，必须排除。其他非文件工具（`fetch_url`、`activate_skill`、`run_agent`）对 dream 无用，排除可减少模型分心。

## Risks / Trade-offs

**[风险] Dream 消息长期占用上下文** → Whisper 消息不会被 `/compact` 压缩，dream 历史会持续累积。这是隐私隔离的必要代价，用户可通过开新 session 释放上下文。

**[风险] Agent 可能误操作非目标层文件** → 由 prompt 引导目录边界，agent 在正常对话中本来就有两层 memory 访问权，dream 时写错层不构成安全问题。`shell` 已通过 `exclude_tools` 排除，防止绕过 approval 直接操作文件。

**[Trade-off] 不支持 `@all` 批量执行** → 为保证 whisper 隔离的完整性，不支持 `@all`。用户可手动逐个 agent 触发，dream 不是高频操作。

**[Trade-off] 不支持自动触发** → 简化实现，用户手动控制。未来可以作为独立 feature 添加。
