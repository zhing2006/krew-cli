## Why

当 Agent 正在流式响应时，用户无法发送新消息——按 Enter 只会插入换行。用户必须等待整个 Agent 调度（包括多 Agent 串行）完成后才能输入下一条消息。这在长时间工具调用或多 Agent 响应时体验很差，打断了用户的思路。

Pending Message 系统让用户可以在 Agent 响应期间预先输入并排队一条消息，Agent 完成后自动提交，实现无缝衔接。

## What Changes

- 新增 pending message 队列（`VecDeque<PendingMessage>`），Agent 响应期间 Enter 将消息入队而非插入换行
- 队列上限常量 `MAX_PENDING_MESSAGES = 1`，队列已满时 Enter 回退为插入换行（当前行为）
- Viewport 顶部动态显示待发送消息区域，有 pending 时视口向上扩展
- 上箭头键（↑）双模式：有 pending 时撤销最后一条到输入区域，无 pending 时调取输入历史（原有行为）
- Agent 调度完成后自动 drain 队列，逐条提交并等待每条响应完毕

## Capabilities

### New Capabilities
- `pending-message-queue`: Pending message 队列管理——入队、出队、撤销、自动 drain 的核心逻辑
- `pending-message-display`: Pending message 在 viewport 中的渲染——顶部待发送区域的显示与动态高度调整

### Modified Capabilities
- `input-routing`: Enter 键在 Agent 响应期间从插入换行变为入队消息，↑ 键新增撤销 pending 模式
- `multi-agent-dispatch`: Agent 调度完成后触发 pending queue drain，逐条自动提交

## Impact

- **krew-cli crate**: `app/input.rs`（Enter/↑ 键行为）、`app/state.rs`（pending 队列状态 + drain 逻辑）、`app/message.rs`（队列入队 + 提交方法）、`render/viewport.rs`（pending 区域渲染 + 动态高度）
- **krew-core crate**: 无变更——Agent loop 和消息类型不受影响
- **依赖**: 无新依赖
