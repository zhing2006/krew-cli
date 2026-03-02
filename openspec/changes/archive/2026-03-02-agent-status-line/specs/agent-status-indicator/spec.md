## ADDED Requirements

### Requirement: Agent 状态行显示
Agent 活跃处理期间，viewport SHALL 在上分隔线正上方显示一行状态指示行，包含 spinner 动画、agent 显示名、"Working" 文字、已用时间和中断提示。

#### Scenario: 状态行出现
- **WHEN** 收到 `AgentEvent::ResponseStart`
- **THEN** viewport SHALL 在上分隔线上方新增一行状态行，viewport 高度从 4 行扩展为 5 行

#### Scenario: 状态行消失
- **WHEN** 收到 `AgentEvent::Done` 或 `AgentEvent::Error`
- **THEN** 状态行 SHALL 消失，viewport 高度恢复为 4 行

#### Scenario: 状态行内容格式
- **WHEN** 状态行可见
- **THEN** 内容 SHALL 为：`{spinner} {agent_display_name} Working  ({elapsed} · ESC to interrupt)`，其中 spinner 为 `●` 或 `◦`，elapsed 为格式化的已用时间

### Requirement: Spinner 闪烁动画
状态行的 spinner 符号 SHALL 以 600ms 间隔在亮态和暗态之间交替闪烁。

#### Scenario: 亮态显示
- **WHEN** 自 agent 开始以来的毫秒数除以 600 为偶数
- **THEN** spinner SHALL 显示 `●`（正常亮度）

#### Scenario: 暗态显示
- **WHEN** 自 agent 开始以来的毫秒数除以 600 为奇数
- **THEN** spinner SHALL 显示 `◦`（DarkGray 暗色）

### Requirement: 已用时间显示
状态行 SHALL 显示自 agent 开始处理以来的已用时间，使用紧凑格式。

#### Scenario: 秒级显示
- **WHEN** 已用时间为 45 秒
- **THEN** SHALL 显示 `45s`

#### Scenario: 分秒级显示
- **WHEN** 已用时间为 1 分 23 秒
- **THEN** SHALL 显示 `1m 23s`

#### Scenario: 时分级显示
- **WHEN** 已用时间为 1 小时 5 分
- **THEN** SHALL 显示 `1h 05m`

#### Scenario: 零秒显示
- **WHEN** agent 刚开始处理
- **THEN** SHALL 显示 `0s`

### Requirement: 中断提示
状态行 SHALL 显示 ESC 中断提示，告知用户可通过 ESC 键中断当前处理。

#### Scenario: 提示格式
- **WHEN** 状态行可见
- **THEN** 已用时间后 SHALL 显示 ` · ESC to interrupt`，使用 DarkGray 颜色

### Requirement: 状态行样式
状态行各部分 SHALL 使用统一的样式规范。

#### Scenario: 各部分样式
- **WHEN** 状态行渲染
- **THEN** 样式 SHALL 为：
  - spinner `●`：agent 配置的颜色
  - agent 显示名 + "Working"：agent 配置的颜色，加粗
  - 时间和中断提示括号：DarkGray 颜色
  - 整行 2 空格左缩进（与状态栏对齐）
