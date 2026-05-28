## ADDED Requirements

### Requirement: consume_stream 必须聚合 ThinkingBlockDone

Agent loop 的 `consume_stream` 函数 MUST 在每次 LLM 流式调用结束时收集所有 `StreamEvent::ThinkingBlockDone` 事件并按到达顺序保存为一个 `Vec<ThinkingBlock>`，作为本轮 `StreamResult` 的一部分返回给上层。

#### Scenario: 一轮 LLM 调用产生多个 thinking block 时全部保留

- **WHEN** 单次流中先后到达两次 `StreamEvent::ThinkingBlockDone`（例如 interleaved thinking 与 tool_use 交替）
- **THEN** `StreamResult` SHALL 暴露一个 `thinking_blocks` 集合，其长度等于 2 且元素顺序与到达顺序一致

#### Scenario: 流中没有 thinking 事件时 thinking_blocks 为空

- **WHEN** 单次流中只包含 `TextDelta` 与 `ToolCall` 事件
- **THEN** `StreamResult.thinking_blocks` SHALL 是空向量，且 `consume_stream` MUST 不引入任何额外延迟或副作用

### Requirement: 构造下一轮 assistant ChatMessage 时必须挂载聚合后的 thinking_blocks

当 Agent loop 在工具循环中或循环结束时构造代表本轮 LLM 输出的 assistant `ChatMessage` 时，MUST 把 `StreamResult.thinking_blocks` 写入该 `ChatMessage.thinking_blocks` 字段；该 `ChatMessage` 会被同时 push 到 `tool_round_messages` 与 `messages` 两个集合，两份引用 MUST 携带相同的 thinking blocks。

#### Scenario: 工具循环中下一轮请求会带回 thinking blocks

- **WHEN** Agent loop 第 N 轮收到包含 thinking blocks 的 LLM 响应，紧接着进入第 N+1 轮调用
- **THEN** 第 N+1 轮发送的 `messages` 序列中，第 N 轮对应的 assistant `ChatMessage.thinking_blocks` SHALL 非空，且其内容与第 N 轮 `ThinkingBlockDone` 的聚合结果完全一致

#### Scenario: 最终结束态的 assistant message 也保留 thinking_blocks

- **WHEN** 没有更多工具调用、agent loop 把最终响应作为 `AgentEvent::Done` 上报，并通过 persistence 写入 session
- **THEN** 落到 session 的 assistant 消息 SHALL 保留本轮 thinking_blocks（具体持久化形态由 `Storage/Session/thinking-blocks` 能力规定）
