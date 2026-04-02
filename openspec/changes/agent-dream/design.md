## Context

Agent Memory 系统已实现两层持久化存储（Global `.krew/memory/` + Per-Agent `.krew/memory/agents/{name}/`），Agent 通过 prompt 指令使用 `read_file`/`write_file` 工具管理记忆文件。随着使用时间增长，记忆文件会累积冗余、过时、矛盾的条目。需要一个主动整理机制来保持记忆质量。

free-code 的 Dream（Memory Consolidation）系统已验证了这一模式：让 LLM 执行"做梦"式的反思，整理和修剪记忆文件。krew 需要适配多 Agent 和两层存储架构。

## Goals / Non-Goals

**Goals:**
- 提供 `/dream` slash 命令，用户手动触发记忆整理
- 支持 `global`/`agent`/`all` 三种 scope，控制整理边界
- 支持 `@agent` 寻址指定执行者，`agent` scope 支持 `@all`
- 复用现有 Agent Loop 和工具系统，以普通消息方式注入当前会话
- 构建适配两层存储的 consolidation prompt（Orient → Consolidate → Prune）

**Non-Goals:**
- 不实现自动触发（时间门控、会话计数门控）—— 仅手动触发
- 不扫描 session transcripts —— 记忆已包含会话中的重要信息
- 不引入新的 tool 或子进程机制 —— 使用现有 tools
- 不实现进度追踪 UI（如 free-code 的 DreamTask footer pill）

## Decisions

### Decision 1: 消息注入方式 —— 作为普通 user message

Dream prompt 作为一条 user message 注入当前会话历史，指定 agent 正常执行 agent loop（包括 tool call 循环）。与 skill activation 类似，dream 的过程和结果留在会话历史中。

**替代方案：** 独立上下文（forked agent / 临时 loop）—— 不污染会话历史，但需要新的执行路径，复杂度高。手动触发的 dream 是用户有意为之，留在历史中是合理的。

**Rationale:** 简单、复用所有现有基础设施（agent loop、tool approval、TUI rendering）。用户可以看到 dream 的过程和结果，也可以用 `/compact` 压缩掉。

### Decision 2: 命令格式和路由

```
/dream <scope> @<agent>
```

| 格式 | 行为 |
|------|------|
| `/dream global @opus` | opus 整理 `.krew/memory/`（Global MEMORY.md + topic files） |
| `/dream agent @opus` | opus 整理 `.krew/memory/agents/opus/` |
| `/dream agent @all` | 每个 Agent 依次整理各自的私有目录（按 reply_order 串行） |
| `/dream all @opus` | opus 整理 Global + 自己的 agents/opus/ 目录 |
| `/dream` | 显示用法提示 |

约束：
- `global` 和 `all` scope 不允许 `@all`（防止多 Agent 同时修改 Global MEMORY.md）
- `@agent` 必须是已配置且可用的 Agent
- Agent 必须启用 tools（`tools = true`）—— 否则无法读写文件

**替代方案：** 无 scope 参数（总是整理全部）—— 但两层存储需要精细控制，全局记忆整理权应明确授予。

### Decision 3: Consolidation Prompt —— 3 阶段结构

基于 free-code 的 `buildConsolidationPrompt`，去掉 session transcript 扫描，适配两层存储：

```
Phase 1 — Orient: ls 目录 + 读 MEMORY.md + 浏览 topic 文件
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

### Decision 5: `@all` 在 `agent` scope 下的行为

`/dream agent @all` 按 `reply_order` 串行执行：为每个 agent 构建独立的 dream prompt（各自的目录路径），依次注入为 user message。每个 agent 的 dream 是一个完整的消息-回复轮次。

**Rationale:** Per-Agent 目录互不相干，串行执行安全且可控。用户可以在第一个 agent 做完后中断。

## Risks / Trade-offs

**[风险] Dream 消息污染会话上下文** → 可接受。Dream 是用户主动触发的有意行为，结果留在历史中便于审查。如果上下文过长，用户可以 `/compact` 压缩。

**[风险] Agent 可能误解 prompt 做出意外修改** → Approval carve-out 已覆盖 `.krew/memory/**`，但 deny_rules 仍可覆盖。Dream prompt 明确限定操作范围为 memory 目录。

**[风险] `@all` 串行执行可能耗时较长** → `agent` scope 下 `@all` 是唯一允许的批量操作。每个 agent 的私有目录通常较小（只有 feedback 类型），执行应该很快。

**[Trade-off] 不支持自动触发** → 简化实现，用户手动控制。未来可以作为独立 feature 添加。
