## Requirements

### Requirement: Agent 回复 @ 检测
系统 SHALL 扫描 Agent 回复的 `final_text`，遍历文本中所有 `@` 位置，对 `@` 之后的内容使用最长前缀匹配检查已知 Agent name（前缀后的下一个字符须为非字母数字字符或文本结尾）。若 `@` 前一个字符为 ASCII 字母数字字符（如 `email@agent`），SHALL 跳过该 `@`。匹配范围为当前会话中已成功初始化且持有 runtime 的 Agent（即 `self.agents` 键集），排除回复者自身和 `@all`。返回所有匹配的 Agent name（按文本出现顺序，去重）。

#### Scenario: Agent @ 另一个 Agent
- **WHEN** agent "gpt" 的回复文本中包含 `@opus`
- **AND** "opus" 是当前会话中已初始化的 Agent
- **THEN** 系统 SHALL 检测 "opus" 为路由目标

#### Scenario: Agent @ 自己
- **WHEN** agent "gpt" 的回复文本中包含 `@gpt`
- **THEN** 系统 SHALL 忽略此 @（不作为路由目标）

#### Scenario: Agent @ 未知名称
- **WHEN** agent "gpt" 的回复文本中包含 `@unknown`
- **AND** "unknown" 不是已初始化的 Agent
- **THEN** 系统 SHALL 不触发任何 AI-to-AI 路由

#### Scenario: Agent @ 多个 Agent
- **WHEN** agent "gpt" 的回复文本中同时包含 `@opus` 和 `@gemini`
- **THEN** 系统 SHALL 将所有匹配的 Agent 按文本出现顺序作为路由目标（不再仅取第一个）

#### Scenario: Agent 使用 @all
- **WHEN** agent "gpt" 的回复文本中包含 `@all`
- **THEN** 系统 SHALL 不触发 AI-to-AI 路由（`@all` 仅用户可用）

#### Scenario: @ 后紧跟标点符号（ASCII）
- **WHEN** agent "gpt" 的回复文本中包含 `@opus,` 或 `@opus:` 等 @ 后紧跟标点的形式
- **THEN** 系统 SHALL 通过前缀匹配检测 "opus" 为路由目标

#### Scenario: CJK 标点紧邻 @ 前方
- **WHEN** agent "gpt" 的回复文本中包含 `太好了！@gemini`（全角标点紧邻 `@`，无空格分隔）
- **AND** "gemini" 是已知 Agent
- **THEN** 系统 SHALL 正确检测 "gemini" 为路由目标（`@` 前方的非 ASCII 字符不视为 email 前缀）

#### Scenario: 非 ASCII agent name 带 CJK 标点
- **WHEN** agent "gpt" 的回复文本中包含 `@助手，你觉得呢`（无空格分隔）
- **AND** "助手" 是已知 Agent
- **THEN** 系统 SHALL 通过前缀匹配检测 "助手" 为路由目标

#### Scenario: Email 地址中的 @ 不误命中
- **WHEN** agent "gpt" 的回复文本中包含 `user@opus.com`
- **AND** "opus" 是已知 Agent
- **THEN** 系统 SHALL 不触发路由（`@` 前方为 ASCII 字母数字字符，视为 email 地址）

#### Scenario: 前缀匹配不误命中更长的名字
- **WHEN** 已知 Agent 列表包含 "opus" 和 "opusX"
- **AND** agent "gpt" 的回复文本中包含 `@opusX`
- **THEN** 系统 SHALL 匹配 "opusX" 而非 "opus"（前缀后的下一个字符为字母数字时不匹配短名）

### Requirement: Immediate 路由策略（cursor 模式）
当 `agent_to_agent_routing` 设为 `immediate` 时，系统 SHALL 使用 `a2a_insert_cursor` 管理插入位置，确保新的 a2a 目标不会跳到已有 a2a 条目之前。

#### Scenario: 单目标——插入 cursor 位置
- **WHEN** 路由策略为 `immediate`
- **AND** `pending_agents` 为空，cursor 为 0
- **AND** 目标 Agent "a" 不在队列中
- **THEN** 系统 SHALL 在 cursor 位置插入 "a"，cursor 变为 1
- **AND** 队列为 `[a]`

#### Scenario: 多目标——按文本顺序依次插入
- **WHEN** 路由策略为 `immediate`
- **AND** Agent "c" 的回复 @a @b
- **THEN** 系统 SHALL 依次在 cursor 位置插入 "a"（cursor 0→1）、"b"（cursor 1→2）
- **AND** 队列为 `[a, b]`

#### Scenario: 防止饥饿——后续 a2a 不跳过已有 a2a
- **WHEN** 队列为 `[b]`（cursor=1），A 的回复 @c
- **THEN** 系统 SHALL 在 cursor 位置 1 插入 "c"
- **AND** 队列为 `[b, c]`（b 不被跳过）

#### Scenario: 目标已在队列中——移动到 cursor 位置
- **WHEN** 路由策略为 `immediate`
- **AND** 目标 Agent 已在队列中但不在 cursor 位置
- **THEN** 系统 SHALL 将目标移动到 cursor 位置

#### Scenario: 目标已在 cursor 位置
- **WHEN** 路由策略为 `immediate`
- **AND** 目标 Agent 已在 cursor 位置
- **THEN** 系统 SHALL 不改变队列，cursor 前进 1

#### Scenario: cursor 超限自动裁剪
- **WHEN** cursor 值大于队列长度
- **THEN** 系统 SHALL 将 cursor 裁剪为队列长度后再插入

#### Scenario: Pop 时递减 cursor
- **WHEN** `start_next_agent` 从队列头部弹出一个 Agent
- **THEN** 系统 SHALL 将 cursor 减 1（saturating_sub）

### Requirement: 原始 Immediate 路由（无 cursor）
保留 `apply_immediate_routing` 函数用于不需要 cursor 追踪的场景。行为与此前一致：目标插入/移动到队列头部。

### Requirement: Queued 路由策略
当 `agent_to_agent_routing` 设为 `queued` 时，系统 SHALL 将目标 Agent 追加到队列尾部，不改变已有顺序。

#### Scenario: 目标已在队列中——不变
- **WHEN** 路由策略为 `queued`
- **AND** 目标 Agent "opus" 已在 `pending_agents` 队列中
- **THEN** 系统 SHALL 不改变队列

#### Scenario: 目标不在队列中——追加尾部
- **WHEN** 路由策略为 `queued`
- **AND** 目标 Agent "opus" 不在 `pending_agents` 队列中
- **THEN** 系统 SHALL 将 "opus" 追加到 `pending_agents` 尾部

### Requirement: 轮次限制
系统 SHALL 跟踪当前用户消息轮次内的 AI-to-AI 路由触发次数，并强制执行可配置的最大值。多目标场景下，每个目标独立计数。

#### Scenario: 未超限
- **WHEN** AI-to-AI 路由被触发
- **AND** 当前轮次计数小于 `agent_to_agent_max_rounds`
- **THEN** 系统 SHALL 递增轮次计数器并执行路由

#### Scenario: 超限
- **WHEN** AI-to-AI 路由将被触发
- **AND** 当前轮次计数等于 `agent_to_agent_max_rounds`
- **THEN** 系统 SHALL 不插入/移动目标 Agent
- **AND** SHALL 显示提示信息（AI-to-AI 轮次已达上限）
- **AND** SHALL 让 `pending_agents` 中剩余 Agent 正常执行

#### Scenario: 多目标部分超限
- **WHEN** Agent 回复中 @a @b @c
- **AND** 处理 @b 时轮次计数达到上限
- **THEN** 系统 SHALL 路由 @a 和 @b，跳过 @c
- **AND** SHALL 显示提示信息

#### Scenario: 用户发消息时重置计数器
- **WHEN** 用户发送新消息
- **THEN** 系统 SHALL 将 AI-to-AI 轮次计数器和 a2a_insert_cursor 重置为 0

### Requirement: max_rounds 为 0 时禁用功能
当 `agent_to_agent_max_rounds` 设为 0 时，AI-to-AI 路由功能 SHALL 完全禁用。

#### Scenario: 设为 0 时禁用
- **WHEN** `agent_to_agent_max_rounds` 为 0
- **AND** Agent 回复中包含 `@other_agent`
- **THEN** 系统 SHALL 不执行任何 AI-to-AI 路由
- **AND** 行为 SHALL 与 v0.4 之前完全一致

### Requirement: System prompt Agent 列表注入
当 AI-to-AI 路由功能启用时，系统 SHALL 在每个 Agent 的 system prompt 中注入当前会话中其他已初始化 Agent 的名称列表和 @ 用法说明。提示词 SHALL 明确要求：(1) `@name` 前后需要空格；(2) 仅在需要对方回复时才使用 `@`，提及 Agent 名字时不加 `@`。

#### Scenario: 启用时注入
- **WHEN** `agent_to_agent_max_rounds` > 0
- **THEN** Agent 的 system prompt SHALL 包含当前会话中其他已初始化 Agent 的 `[name] display_name` 列表及 `@agent_name` 协作说明

#### Scenario: 禁用时不注入
- **WHEN** `agent_to_agent_max_rounds` 为 0
- **THEN** Agent 的 system prompt SHALL 不包含任何 AI-to-AI 相关指引

#### Scenario: 不可用 Agent 不注入
- **WHEN** 配置中存在 Agent "gemini" 但因 API Key 缺失未成功初始化
- **THEN** system prompt 中 SHALL 不包含 "gemini"

### Requirement: 配置默认值
系统 SHALL 为 AI-to-AI 配置字段提供默认值。

#### Scenario: 路由策略默认值
- **WHEN** `agent_to_agent_routing` 未在 settings 中指定
- **THEN** 系统 SHALL 默认为 `immediate`

#### Scenario: 最大轮次默认值
- **WHEN** `agent_to_agent_max_rounds` 未在 settings 中指定
- **THEN** 系统 SHALL 默认为 `10`

### Requirement: AI-to-AI 期间用户观看模式
在 AI-to-AI 对话进行期间，用户 SHALL 不能发送新消息，但可以在输入框中编辑内容。用户 SHALL 保留通过 ESC 中断当前 AI-to-AI 链路的能力。

#### Scenario: ESC 取消 AI-to-AI 对话链
- **WHEN** AI-to-AI 对话正在进行
- **AND** 用户按下 ESC
- **THEN** 系统 SHALL 取消当前 Agent 回复并清空 pending 队列（与现有 ESC 行为一致）
- **AND** SHALL 重置 AI-to-AI 轮次计数器和 a2a_insert_cursor
