## ADDED Requirements

### Requirement: Dream 命令格式
`/dream` 命令 SHALL 接受以下格式：

```
/dream <scope> @<agent>
```

其中 `<scope>` 为 `global`、`agent` 或 `all`，`<agent>` 为目标 Agent 名称或 `all`。

#### Scenario: 完整命令
- **WHEN** 用户输入 `/dream global @opus`
- **THEN** 系统 SHALL 解析 scope 为 `global`，agent 为 `opus`

#### Scenario: agent scope 批量
- **WHEN** 用户输入 `/dream agent @all`
- **THEN** 系统 SHALL 解析 scope 为 `agent`，agent 为 `all`（表示所有 Agent 依次执行）

#### Scenario: 无参数
- **WHEN** 用户输入 `/dream`
- **THEN** 系统 SHALL 在 viewport 上方显示用法提示信息

#### Scenario: 缺少 agent 参数
- **WHEN** 用户输入 `/dream global`（无 `@agent`）
- **THEN** 系统 SHALL 在 viewport 上方显示用法提示信息

### Requirement: Scope 与 @all 约束
`global` 和 `all` scope SHALL 禁止 `@all` 寻址。仅 `agent` scope 允许 `@all`。

#### Scenario: global scope 使用 @all
- **WHEN** 用户输入 `/dream global @all`
- **THEN** 系统 SHALL 显示错误：`@all` 仅可用于 `agent` scope

#### Scenario: all scope 使用 @all
- **WHEN** 用户输入 `/dream all @all`
- **THEN** 系统 SHALL 显示错误：`@all` 仅可用于 `agent` scope

### Requirement: Agent 校验
指定的 Agent SHALL 存在于当前配置中且已成功初始化。Agent SHALL 启用 tools（`tools = true`），否则无法执行记忆文件操作。

#### Scenario: Agent 不存在
- **WHEN** 用户输入 `/dream global @unknown`
- **THEN** 系统 SHALL 显示错误：Agent "unknown" not found

#### Scenario: Agent 未启用 tools
- **WHEN** 用户输入 `/dream global @reader` 且 `reader` 的 `tools = false`
- **THEN** 系统 SHALL 显示错误提示 Agent 未启用 tools，无法执行 dream

### Requirement: Consolidation Prompt 构建
系统 SHALL 根据 scope 构建 consolidation prompt，包含 3 个阶段：

1. **Phase 1 — Orient**: 指示 Agent 列出目录内容、读取 MEMORY.md、浏览已有 topic 文件
2. **Phase 2 — Consolidate**: 指示 Agent 合并重复文件、删除陈旧事实、修复矛盾条目、转换相对日期为绝对日期
3. **Phase 3 — Prune index**: 指示 Agent 更新 MEMORY.md 使其保持在 200 行 / 25KB 以内

#### Scenario: global scope prompt
- **WHEN** scope 为 `global`
- **THEN** prompt SHALL 仅包含 `.krew/memory/` 目录路径，不包含 Per-Agent 目录

#### Scenario: agent scope prompt
- **WHEN** scope 为 `agent`，agent 为 `opus`
- **THEN** prompt SHALL 仅包含 `.krew/memory/agents/opus/` 目录路径，不包含 Global 目录

#### Scenario: all scope prompt
- **WHEN** scope 为 `all`，agent 为 `opus`
- **THEN** prompt SHALL 包含 `.krew/memory/` 和 `.krew/memory/agents/opus/` 两个目录，并明确指出两个 MEMORY.md 是独立索引

### Requirement: Dream 消息注入
Dream prompt SHALL 以 user message 方式注入当前会话历史，指定 Agent 正常执行 agent loop（包括 tool call 循环）。

#### Scenario: 单 Agent dream
- **WHEN** 用户执行 `/dream global @opus`
- **THEN** 系统 SHALL 构建 global scope 的 consolidation prompt，作为 user message 追加到会话历史，并将 `opus` 加入 agent dispatch 队列

#### Scenario: agent scope @all 串行执行
- **WHEN** 用户执行 `/dream agent @all` 且 reply_order 为 `["gpt", "opus"]`
- **THEN** 系统 SHALL 按 reply_order 依次为每个 Agent 注入独立的 dream prompt（各自的目录路径），每个 Agent 完成一个完整的消息-回复轮次

### Requirement: Dream 消息的 addressee
Dream 注入的 user message SHALL 设置 addressee 为目标 Agent 名称，确保其他 Agent 在后续轮次中能看到 dream 的上下文。

#### Scenario: 消息 addressee 设置
- **WHEN** 系统为 `opus` 构建 dream user message
- **THEN** 该 message 的 addressee SHALL 为 `opus`

### Requirement: Dream 执行状态提示
系统 SHALL 在 dream 开始时显示状态提示信息。

#### Scenario: 单 Agent 状态提示
- **WHEN** 用户执行 `/dream global @opus`
- **THEN** 系统 SHALL 在 viewport 上方显示类似 `Dreaming with [opus] (global)...` 的提示

#### Scenario: @all 状态提示
- **WHEN** 用户执行 `/dream agent @all`
- **THEN** 系统 SHALL 在每个 Agent 开始 dream 时显示类似 `Dreaming with [gpt] (agent)...` 的提示
