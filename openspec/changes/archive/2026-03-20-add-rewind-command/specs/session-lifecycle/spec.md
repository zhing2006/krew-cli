## ADDED Requirements

### Requirement: Rewound 状态标记
系统 SHALL 维护一个 `rewound` 布尔标记，用于控制 rewind 后的 session 保存行为。

#### Scenario: Rewind 设置标记
- **WHEN** rewind 操作完成
- **THEN** `rewound` SHALL 被设置为 `true`

#### Scenario: 发新消息时清除标记并换 session ID
- **WHEN** 用户发送消息且 `rewound` 为 `true`
- **THEN** 系统 SHALL 生成新 session ID 和 session_created_at，将 `rewound` 设为 `false`，然后正常处理消息

#### Scenario: save_session() 统一守卫
- **WHEN** `rewound` 为 `true` 且任何代码路径调用 `save_session()`
- **THEN** `save_session()` SHALL 静默返回，不执行任何写盘操作

#### Scenario: /resume 清除 rewound 状态
- **WHEN** `rewound` 为 `true` 且用户通过 `/resume` 成功加载另一个 session
- **THEN** `rewound` SHALL 被设为 `false`，加载的 session 不继承 rewound 状态

#### Scenario: /clear 清除 rewound 状态
- **WHEN** `rewound` 为 `true` 且用户执行 `/clear`
- **THEN** `rewound` SHALL 被设为 `false`

### Requirement: /clear 重置 session_created_at
`execute_new()` SHALL 在生成新 session_id 时同步重置 `session_created_at` 为当前时间。

#### Scenario: /clear 后 session_created_at 更新
- **WHEN** 用户执行 `/clear`（无论是否处于 rewound 状态）
- **THEN** `session_created_at` SHALL 被重置为 `Utc::now()`，确保新会话的创建时间正确
