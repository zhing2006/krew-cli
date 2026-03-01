## ADDED Requirements

### Requirement: HTTP 状态码分类
`krew-llm` SHALL 在 `common.rs` 模块中定义 `RetryAction` 枚举（`RateLimit`、`ServerError`、`AuthError`、`NoRetry`）和 `classify_status(StatusCode) -> RetryAction` 函数。

#### Scenario: 429 分类为 RateLimit
- **WHEN** 输入状态码 429
- **THEN** SHALL 返回 `RetryAction::RateLimit`

#### Scenario: 5xx 分类为 ServerError
- **WHEN** 输入状态码 500、502、503
- **THEN** SHALL 返回 `RetryAction::ServerError`

#### Scenario: 401/403 分类为 AuthError
- **WHEN** 输入状态码 401 或 403
- **THEN** SHALL 返回 `RetryAction::AuthError`

#### Scenario: 其他状态码分类为 NoRetry
- **WHEN** 输入状态码 400、404 等其他客户端错误
- **THEN** SHALL 返回 `RetryAction::NoRetry`

### Requirement: 错误消息提取
`common.rs` SHALL 定义 `extract_error_message(Response) -> String` 异步函数，从 HTTP 错误响应中提取错误信息。

#### Scenario: JSON 错误体
- **WHEN** 响应 body 为 `{"error": {"message": "rate limit exceeded"}}`
- **THEN** SHALL 返回 `"{status}: rate limit exceeded"` 格式的字符串

#### Scenario: 非 JSON 错误体
- **WHEN** 响应 body 不是有效 JSON
- **THEN** SHALL 返回 `"{status}: {raw_body}"` 格式的字符串

### Requirement: 带重试的请求发送
`common.rs` SHALL 定义通用的 `send_with_retry` 异步函数，接受 HTTP 请求构建器和重试配置，实现 429 指数退避重试（最多 3 次）、5xx 固定间隔重试（最多 2 次，间隔 2s）、超时重试（60s 后重试 1 次）。

#### Scenario: 429 指数退避
- **WHEN** 请求返回 429
- **THEN** SHALL 以 1s → 2s → 4s 退避重试，最多 3 次

#### Scenario: 5xx 固定间隔
- **WHEN** 请求返回 500
- **THEN** SHALL 以 2s 间隔重试，最多 2 次

#### Scenario: 超时重试
- **WHEN** 请求在 60 秒内未响应
- **THEN** SHALL 重试 1 次，若仍超时返回 `LlmError::Api`

#### Scenario: 401 不重试
- **WHEN** 请求返回 401
- **THEN** SHALL 立即返回 `LlmError::Auth`，不重试

### Requirement: openai_chat.rs 迁移到公共模块
`openai_chat.rs` 中的 `classify_status`、`extract_error_message`、`send_with_retry` 逻辑 SHALL 迁移到 `common.rs`，`openai_chat.rs` SHALL 调用公共函数替代内联实现。迁移后行为 SHALL 与原实现完全一致。

#### Scenario: openai_chat 现有测试仍通过
- **WHEN** 迁移完成后运行 `openai_chat` 的全部现有测试
- **THEN** 所有测试 SHALL 通过，无行为变化

### Requirement: 连续同 Role 消息合并
`common.rs` SHALL 定义 `merge_consecutive_same_role` 函数，接受已转换 role 后的消息数组，合并连续相同 role 的消息。合并时用 `\n\n` 连接 content。

#### Scenario: 两条连续 user 消息合并
- **WHEN** 输入 `[{role:"user", content:"[agentA] foo"}, {role:"user", content:"[agentB] bar"}]`
- **THEN** SHALL 输出 `[{role:"user", content:"[agentA] foo\n\n[agentB] bar"}]`

#### Scenario: 交替消息不合并
- **WHEN** 输入 `[{role:"user", content:"hi"}, {role:"assistant", content:"hello"}]`
- **THEN** SHALL 原样输出两条消息
