## ADDED Requirements

### Requirement: Sub-Agent 定义文件格式
系统 SHALL 支持 Markdown + YAML frontmatter 格式的 Sub-Agent 定义文件。YAML frontmatter 中 `name`（字符串）和 `description`（字符串）为 MUST 字段。文件 body（frontmatter 之后的内容）SHALL 作为 Sub-Agent 的 system prompt。

`color`（字符串）和 `maxTurns`（整数）为可选字段，分别用于 TUI 显示颜色和 agent loop 最大轮次（默认值 30）。

Claude Code 兼容字段（`tools`、`model`、`disallowedTools`、`permissionMode`、`skills`、`mcpServers`、`hooks`、`memory`、`background`、`effort`、`isolation`、`initialPrompt`、`provider`）SHALL 被解析但忽略，不影响 Sub-Agent 行为。

#### Scenario: 解析有效的 Sub-Agent 定义文件
- **WHEN** 系统读取包含 `name: git` 和 `description: Git operations agent` 的 YAML frontmatter 及 body 内容的 `.md` 文件
- **THEN** 系统 SHALL 返回 `SubAgentDef`，其 `name` 为 `"git"`，`description` 为 `"Git operations agent"`，`system_prompt` 为 body 内容

#### Scenario: 解析缺少必需字段的定义文件
- **WHEN** 系统读取缺少 `name` 或 `description` 字段的 `.md` 文件
- **THEN** 系统 SHALL 跳过该文件并记录 warning 日志

#### Scenario: 解析包含 Claude Code 兼容字段的定义文件
- **WHEN** 系统读取包含 `tools: Bash, Read` 和 `model: inherit` 等 Claude Code 字段的 `.md` 文件
- **THEN** 系统 SHALL 正常解析 `name` 和 `description`，忽略不支持的字段

#### Scenario: 解析包含 color 和 maxTurns 的定义文件
- **WHEN** 系统读取包含 `color: cyan` 和 `maxTurns: 50` 的 `.md` 文件
- **THEN** 系统 SHALL 在 `SubAgentDef` 中设置 `color` 为 `"cyan"`，`max_turns` 为 `50`

### Requirement: Sub-Agent 定义文件发现
系统 SHALL 从以下 6 个路径（按优先级从高到低）扫描 `*.md` 文件作为 Sub-Agent 定义：

1. `<cwd>/.krew/agents/` — 项目级，krew-specific
2. `<cwd>/.agents/agents/` — 项目级，cross-client
3. `<cwd>/.claude/agents/` — 项目级，Claude Code 兼容
4. `<home>/.krew/agents/` — 用户级，krew-specific
5. `<home>/.agents/agents/` — 用户级，cross-client
6. `<home>/.claude/agents/` — 用户级，Claude Code 兼容

扫描 SHALL 为非递归（只扫描目录顶层的 `.md` 文件）。当同名 Sub-Agent 出现在多个路径时，SHALL 使用高优先级路径的版本（first-found-wins）。

#### Scenario: 从多个目录发现 Sub-Agent 定义
- **WHEN** `<cwd>/.krew/agents/git.md` 和 `<cwd>/.claude/agents/git.md` 都定义了 `name: git`
- **THEN** 系统 SHALL 使用 `.krew/agents/git.md` 的定义（优先级更高）

#### Scenario: 目录不存在时跳过
- **WHEN** 某个扫描路径不存在
- **THEN** 系统 SHALL 跳过该路径，不报错

#### Scenario: 发现多个不同名的 Sub-Agent
- **WHEN** `<cwd>/.claude/agents/` 下存在 `git.md`（name: git）和 `researcher.md`（name: researcher）
- **THEN** 系统 SHALL 返回包含两个 `SubAgentDef` 的列表

### Requirement: Sub-Agent Catalog 注入
当发现了至少一个 Sub-Agent 定义时，系统 SHALL 在所有 Peer Agent 的 system prompt 中注入 Sub-Agent catalog，格式为 XML：

```xml
<available-sub-agents>
  <agent name="git">Git operations agent...</agent>
  <agent name="researcher">Research agent...</agent>
</available-sub-agents>
```

此 catalog SHALL 让 LLM 了解可用的 Sub-Agent 及其用途，以便决定何时调用 `run_agent` tool。

#### Scenario: 没有 Sub-Agent 定义时不注入
- **WHEN** 所有扫描路径都没有发现 Sub-Agent 定义
- **THEN** 系统 SHALL 不注入 Sub-Agent catalog，也不注册 `run_agent` tool

#### Scenario: 有 Sub-Agent 定义时注入 catalog
- **WHEN** 发现了 `git` 和 `researcher` 两个 Sub-Agent 定义
- **THEN** 系统 SHALL 在每个 Peer Agent 的 system prompt 中追加包含这两个 Agent 描述的 XML catalog
