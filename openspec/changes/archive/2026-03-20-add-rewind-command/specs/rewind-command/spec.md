## ADDED Requirements

### Requirement: /rewind 命令入口
`/rewind` SHALL 是一个内置 slash 命令，在当前 session 有用户消息时弹出 RewindPicker popup。

#### Scenario: 有用户消息时执行 /rewind
- **WHEN** 用户输入 `/rewind` 且当前 session 包含至少一条 role=User 的消息
- **THEN** 系统 SHALL 弹出 RewindPicker popup，列出所有用户消息供选择

#### Scenario: 无用户消息时执行 /rewind
- **WHEN** 用户输入 `/rewind` 且当前 session 没有 role=User 的消息
- **THEN** 系统 SHALL 显示 info 提示 "Nothing to rewind — no messages yet"

### Requirement: RewindPicker popup 展示
RewindPicker popup SHALL 按时间正序展示所有用户消息，默认选中最后一条。每个 popup item 的 `value` 字段 SHALL 存储该用户消息在 `self.messages` 中的原始下标（转为字符串），以确保选择后能精确定位截断位置。

#### Scenario: Popup 列表内容
- **WHEN** RewindPicker popup 弹出
- **THEN** 列表 SHALL 按时间正序排列（最早在前），每项的 `description` 显示 `时间  "内容预览"`，内容预览截断到 40 字符

#### Scenario: Popup 默认选中最后一条
- **WHEN** RewindPicker popup 弹出
- **THEN** 列表 SHALL 默认选中最后一条用户消息（即最近的一轮），用户直接按 Enter 即可回退最近一轮

#### Scenario: Popup item value 持有原始下标
- **WHEN** popup item 被构建
- **THEN** 每个 item 的 `value` SHALL 是该用户消息在 `self.messages` 数组中的原始索引（转为字符串形式），而非显示序号或用户消息序数

#### Scenario: Popup 交互
- **WHEN** popup 显示时
- **THEN** 用户 SHALL 可以使用上下键浏览、Enter 确认选择、ESC 取消

### Requirement: 选择后截断对话
用户在 RewindPicker 中选择某条消息后，系统 SHALL 根据 item value 中的原始下标截断对话历史。

#### Scenario: 截断到中间节点
- **WHEN** 用户选择了原始下标为 N 的用户消息（N > 0）
- **THEN** 系统 SHALL 执行 `self.messages.truncate(N)`，保留 messages[0..N]，丢弃 messages[N] 及之后的所有消息

#### Scenario: 选择第一条用户消息等同 /clear
- **WHEN** 用户选择的用户消息原始下标为 0
- **THEN** 系统 SHALL 直接调用 `/clear` 逻辑（`execute_new()`），正常保存当前完整 session、清屏、生成新 session ID；SHALL NOT 设置 `rewound`，SHALL NOT 执行截断

### Requirement: 截断后屏幕重放
截断后系统 SHALL 清屏并重放截断后保留的对话历史。

#### Scenario: 清屏和重放
- **WHEN** 对话被截断（且截断后仍有消息）
- **THEN** 系统 SHALL 清屏、显示 header、按原始格式重放所有保留的消息（用户消息、agent header、assistant 回复、tool 调用等）

#### Scenario: 重放后显示提示
- **WHEN** 重放完成
- **THEN** 系统 SHALL 在 viewport 上方显示 info 提示，告知用户已回退

### Requirement: Fork 语义——延迟生成新 session ID
当用户选择非第一条用户消息进行 rewind 时，系统 SHALL 进入 fork 语义：不立即保存 session 文件，原始 session 文件保持不变，直到用户发送新消息。选择第一条用户消息时不进入 fork 语义（见"选择第一条用户消息等同 /clear" scenario）。

#### Scenario: Rewind 到非第一条消息后不保存
- **WHEN** rewind 截断到非第一条用户消息（截断后仍有消息）
- **THEN** 磁盘上的原始 session 文件 SHALL 保持不变

#### Scenario: Rewind 后发新消息产生新 session
- **WHEN** 用户在 rewind（非第一条）后发送第一条新消息
- **THEN** 系统 SHALL 生成新的 session ID 和 session_created_at，然后正常保存 session

#### Scenario: Rewind 后执行 /exit 不覆盖原始 session
- **WHEN** 用户在 rewind 后直接执行 `/exit`（未发送新消息）
- **THEN** 系统 SHALL NOT 将截断后的内容保存到原始 session 文件

#### Scenario: Rewind 后执行 /clear
- **WHEN** 用户在 rewind 后执行 `/clear`
- **THEN** 系统 SHALL NOT 保存截断后的 session，SHALL 清除 rewound 标记并生成新 session ID 开始全新会话

#### Scenario: Rewind 后执行 /resume
- **WHEN** 用户在 rewind 后执行 `/resume` 并成功加载另一个 session
- **THEN** 系统 SHALL NOT 保存截断后的 session，SHALL 在加载成功后清除 `rewound` 标记，加载的 session 不继承 rewound 状态

#### Scenario: Rewind 后执行 /compact
- **WHEN** 用户在 rewind 后执行 `/compact`（`rewound` 为 true）
- **THEN** 系统 SHALL 拒绝执行并显示提示信息，告知用户需先发送新消息再 compact

### Requirement: 状态重建
Rewind 截断后，系统 SHALL 从保留的消息重新推导相关状态。

#### Scenario: Token usage 重建
- **WHEN** 对话被截断
- **THEN** `agent_token_usage` SHALL 从截断后的消息重新计算，取每个 agent 最后一条 assistant 消息的 usage

#### Scenario: Last respondent 重建
- **WHEN** 对话被截断
- **THEN** `last_respondent` SHALL 更新为截断后消息中最后一条 assistant 消息的 agent_name

#### Scenario: Skill activation state 重建
- **WHEN** 对话被截断
- **THEN** 系统 SHALL 从截断后的消息重新推导已激活的 skills 并重置 session-scoped tool state
