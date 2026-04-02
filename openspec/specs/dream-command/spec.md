## Requirements

### Requirement: Dream 命令格式
`/dream` 命令 SHALL 接受以下格式：

```
/dream <scope> @<agent>
```

其中 `<scope>` 为 `global`、`agent` 或 `all`，`<agent>` 为目标 Agent 名称（不支持 `all`）。

#### Scenario: 完整命令
- **WHEN** 用户输入 `/dream global @opus`
- **THEN** 系统 SHALL 解析 scope 为 `global`，agent 为 `opus`

#### Scenario: 无参数
- **WHEN** 用户输入 `/dream`
- **THEN** 系统 SHALL 在 viewport 上方显示用法提示信息

#### Scenario: 缺少 agent 参数
- **WHEN** 用户输入 `/dream global`（无 `@agent`）
- **THEN** 系统 SHALL 在 viewport 上方显示用法提示信息

### Requirement: @all 禁止
所有 scope SHALL 禁止 `@all` 寻址。每次 dream 只允许指定单个 Agent。

#### Scenario: 使用 @all
- **WHEN** 用户输入 `/dream <any_scope> @all`
- **THEN** 系统 SHALL 显示错误：`/dream` 不支持 `@all`，请指定单个 Agent

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

1. **Phase 1 — Orient**: 指示 Agent 使用 glob 工具列出目录内容、读取 MEMORY.md、浏览已有 topic 文件
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

### Requirement: Dream 消息注入 —— Whisper 模式
Dream prompt SHALL 以 **whisper message** 方式注入当前会话历史，whisper_targets 设为目标 Agent，确保 dream 过程（包括 tool call/result）对其他 Agent 不可见。

#### Scenario: 单 Agent dream
- **WHEN** 用户执行 `/dream global @opus`
- **THEN** 系统 SHALL 构建 global scope 的 consolidation prompt，作为 whisper message（whisper_targets = ["opus"]）追加到会话历史，设置 addressee 为 `opus`，并将 `opus` 加入 agent dispatch 队列

#### Scenario: Dream 对其他 Agent 不可见
- **WHEN** `opus` 完成 dream 后，后续轮次中 `sonnet` 参与对话
- **THEN** `sonnet` SHALL 看到 whisper placeholder（`[Whisper to opus]`），不可见 dream 过程中的 tool call/result 内容

### Requirement: 工具集收窄
Dream 执行期间，系统 SHALL 通过 `exclude_tools` 限制可用工具，仅保留文件操作相关工具。

#### Scenario: 排除非文件工具
- **WHEN** 系统为 dream 调用 `start_completion()`
- **THEN** `exclude_tools` SHALL 包含 `["shell", "fetch_url", "activate_skill", "run_agent"]`，agent 仅可使用 `read_file`、`write_file`、`edit_file`、`glob`、`grep`（以及 provider-native 的 `web_search`，如果 agent 已配置）

#### Scenario: shell 不可用
- **WHEN** dream 执行期间 agent 尝试调用 `shell` 工具
- **THEN** 该工具 SHALL 不存在于工具列表中，LLM 无法调用

#### Scenario: web_search 保留
- **WHEN** agent 配置了 `enable_web_search = true`
- **THEN** dream 执行期间 provider-native 的 web_search SHALL 保持可用

### Requirement: Dream 执行状态提示
系统 SHALL 在 dream 开始时显示状态提示信息。

#### Scenario: 状态提示
- **WHEN** 用户执行 `/dream global @opus`
- **THEN** 系统 SHALL 在 viewport 上方显示类似 `Dreaming with [opus] (global)...` 的提示
