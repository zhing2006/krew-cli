## Context

当前 krew-cli 的输入流在 `app/input.rs` 中通过 `agent_event_rx.is_some()` 检查来阻塞用户发送消息——Agent 响应期间 Enter 键只能插入换行。TUI 本身是全异步的（`tokio::select!` 多路复用终端事件、Agent 事件、commit tick、draw frame），输入从未被真正阻塞，只是发送逻辑被跳过了。

参考了 Claude Code（TypeScript，三级优先级队列 + useSyncExternalStore）和 Codex CLI（Rust，VecDeque + PendingInputPreview widget + agent_turn_running 门控）的实现。krew-cli 的独特性在于多 Agent 串行调度（`pending_agents: VecDeque<String>`）和 `@`/`#` 路由系统。

## Goals / Non-Goals

**Goals:**
- 用户在 Agent 响应期间可以预排队一条消息，Agent 完成后自动提交
- Viewport 顶部动态显示待发送消息，有 pending 时视口向上扩展
- 上箭头键在有 pending 时变为撤销（pop 回 textarea），无 pending 时保持原有历史行为
- 队列已满时 Enter 回退为换行（当前行为），无提示、无打断

**Non-Goals:**
- 不实现优先级系统（Claude Code 的 now/next/later）——krew-cli 没有中途 steer 的需求
- 不实现中断当前 Agent（pending message 等待调度完成后提交，不打断正在进行的回复）
- 不实现批量提交——逐条提交，每条等待完整响应后再提交下一条
- 不修改 krew-core——所有变更局限在 krew-cli crate 内

## Decisions

### Decision 1: 队列数据结构

**选择**: `VecDeque<PendingMessage>` + `MAX_PENDING_MESSAGES` 常量

**PendingMessage 结构体:**
```rust
struct PendingMessage {
    raw_input: String,
}
```

只存原始输入，不做预解析。原因：`parse_input` 依赖 `known_agents` 等运行时状态，提交时重新解析更可靠。

**替代方案:**
- 预解析并存储 `Addressee`/`body`/`is_whisper`：增加复杂度，且在 agents 动态变化场景下可能不一致
- 用 `String` 直接存储而不包装：缺乏扩展性，未来如果需要添加 metadata（如入队时间）不方便

**选择简单包装的原因:** 当前只存 `raw_input`，但 struct 预留了未来扩展空间（如支持图片输入等），几乎零成本。

### Decision 2: 队列上限为常量 1

**选择**: `const MAX_PENDING_MESSAGES: usize = 1`

常量而非配置项。理由：
- 一条 pending 足够覆盖"预输入下一个问题"的场景
- 多条 pending 容易导致对话混乱（Agent 依次回复多条积压消息，用户失去上下文）
- 如果需要补充内容，按 ↑ 撤销后编辑即可
- 未来如有需求只需改常量值

### Decision 3: Enter 键三态逻辑

**选择**: 在 `handle_key` 的 Enter 分支中实现三态判断

```
Enter (光标在最后一行):
  1. 没有 Agent 运行 → send_message()
  2. Agent 运行中 + pending 未满 → queue_message()
  3. Agent 运行中 + pending 已满 → insert_newline()
```

消息入队时 textarea 清空，给用户即时反馈。入队前做两项验证：
1. 输入非空（去空白后非空）
2. 必须包含 `@` 或 `#` 寻址（解析结果不能是 `LastRespondent`）

验证失败时**保留 textarea 内容不清空**，让用户就地修改。原因：pending 场景下 `LastRespondent` 会随 Agent 串行执行和 A2A 触发而漂移，目标不确定，强制指定可消除歧义。正常发送（Agent 空闲时）不受此限制。

### Decision 4: 上箭头双模式

**选择**: 复用现有上箭头键，根据 pending 状态切换行为

```
↑ (光标在 textarea 第一行):
  有 pending → pop_back() 到 textarea（撤销模式）
  无 pending → 调取输入历史（原有行为）
```

**撤销时 textarea 已有内容的处理:** 直接替换。合并两条含 `@`/`#` 的消息会导致路由器解析为多目标或 `@`/`#` 混用报错，语义完全改变。在 MAX=1 下，入队后 textarea 已清空，几乎不存在草稿；队列满时用户继续写东西，按 ↑ 的意图就是"我要编辑 pending"，覆盖是合理的。

**为什么不用 Ctrl+Z:**
- 上箭头更直觉——"把上面的东西拿下来"
- 不需要额外的清空快捷键，连续按 ↑ 可逐条撤销
- 和 Codex CLI 的 Alt+Up 思路一致，但更简洁

### Decision 5: Pending 区域渲染位置

**选择**: Viewport 最上方，textarea 之上

```
├════════════════════┤ viewport top
│ ┄┄ 待发送 (1) ┄┄┄  │ ← pending area (0~N 行，动态)
│ ⏳ ●● @opus ...   │ ← 带路由指示色点
│ [textarea]         │
│ [status bar]       │
└════════════════════┘ viewport bottom
```

Pending 区域在 `render_input_viewport` 中渲染，紧贴视口顶部。概念上视口高度在现有基础上增加 `pending_area_height`。无 pending 时 pending_area_height = 0，视口与当前完全一致。实现时应在现有 viewport 布局逻辑（含 separator、status bar、popup/overlay 分支）上增量添加 pending 区域高度，而非重写现有高度计算。

Pending 消息使用与用户发送消息相同的路由色点渲染逻辑（调用 `parse_input` 获取 addressee，显示对应 Agent 彩色圆点），前缀 ⏳ 图标区分状态。由于入队时强制要求 `@`/`#` 寻址，色点预览始终准确。

**多行与截断:** Pending 消息截断为单行显示，超出终端宽度的部分用 `…` 截断。每条 pending 恒定占 1 行，高度计算简单可靠。

**Overlay/Popup 分支:** 当 approval overlay 或 completion popup 处于活跃状态时，pending 区域暂时不渲染（不影响队列状态，overlay/popup 关闭后恢复显示）。

### Decision 6: Auto-drain 时机

**选择**: 在 `agent_event_rx` 变为 None 的所有路径上统一触发 drain——包括 Done、Error 和 Cancel（ESC 取消）

```
handle_agent_event(Done/Error) 或 cancel_agent_response()
  → agent_event_rx = None
  → start_next_agent() 或 pending_agents.clear()
    → 返回 false / 队列已空
  → drain_pending_message()
    → pop_front → submit_pending_message()
```

`submit_pending_message` 复用 `send_message` 的核心逻辑（解析、渲染、入历史、dispatch、启动 agent），但输入来源从 textarea 变为 PendingMessage。

三条路径统一：Done 完成、Error 错误、Cancel 取消——只要所有 pending_agents 清空且 agent_event_rx 变为 None，就触发 drain。用户如果想取消 pending，应先按 ↑ 撤销 pending，再按 ESC 取消 agent。

**注意:** 当 completion popup 活跃时，ESC 先关闭 popup 而不会取消 agent（这是已有行为）。只有在无 popup/overlay 时，ESC 才触发 cancel → drain 路径。此行为无需修改，但实现时不要误将 popup-ESC 当作 cancel 处理。

## Risks / Trade-offs

- **[风险] 快速连续 Enter 可能导致竞态** → pending 检查和入队在同一个 `handle_key` 同步调用中完成，不存在竞态
- **[风险] 上箭头双模式可能让用户困惑** → pending 区域明确可见，用户能看到"有东西在排队"，上下文清晰
- **[权衡] 不预解析 pending message** → 渲染时调用 parse_input 做预览解析，显示效果一致，只是不存储解析结果。由于入队强制要求 @/# 寻址，不存在 LastRespondent 漂移问题
- **[权衡] MAX = 1 限制了灵活性** → 常量修改即可放开，当前设计不假设队列长度为 1（VecDeque 支持任意长度）
