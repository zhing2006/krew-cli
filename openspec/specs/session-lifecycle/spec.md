## ADDED Requirements

### Requirement: Session creation on startup
The system SHALL create a new session with a generated UUID when the application starts (unless resuming a previous session).

#### Scenario: Normal startup
- **WHEN** the application starts without `--resume`
- **THEN** a new session SHALL be created with a UUID id, current working directory, configured agent names, empty messages, and current timestamp

#### Scenario: Session ID displayed in header
- **WHEN** a new session is created
- **THEN** the startup header SHALL display the session ID (first 8 characters)

### Requirement: Real-time session persistence
The system SHALL save the session to disk after each message is added to the conversation.

#### Scenario: Save after user message
- **WHEN** the user sends a message
- **THEN** the session file SHALL be updated with the new user message

#### Scenario: Save after agent response
- **WHEN** an agent completes its response (AgentEvent::Done)
- **THEN** the session file SHALL be updated with the assistant message and updated token usage

#### Scenario: Save failure does not crash
- **WHEN** saving the session to disk fails (e.g., disk full, permissions)
- **THEN** the system SHALL log a warning and continue operation without crashing

### Requirement: Session resume from file
The system SHALL restore a session from disk, repopulating the conversation message history.

#### Scenario: Resume restores messages
- **WHEN** a session is resumed (via `/resume` or `--resume`)
- **THEN** the message history SHALL be loaded from the session file and made available for subsequent agent calls

#### Scenario: Resume restores token usage
- **WHEN** a session is resumed
- **THEN** the cumulative token usage per agent SHALL be restored from the session file

#### Scenario: Resume updates session metadata
- **WHEN** a session is resumed
- **THEN** the `updated_at` timestamp SHALL be updated to the current time

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
