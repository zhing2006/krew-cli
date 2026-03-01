## ADDED Requirements

### Requirement: MarkdownStreamCollector 流式收集
`MarkdownStreamCollector` SHALL 累积原始文本 delta，仅在遇到换行符 `\n` 时将完整行通过 Markdown 渲染器转换为 `Vec<Line<'static>>`。

#### Scenario: 累积不含换行的 delta
- **WHEN** 调用 `push_delta("hello ")` 后调用 `push_delta("world")`
- **THEN** SHALL 不产出任何渲染行（内部 buffer 为 `"hello world"`）

#### Scenario: 换行触发渲染
- **WHEN** buffer 中累积了 `"hello **world**\n"` 后调用 `commit_complete_lines()`
- **THEN** SHALL 返回渲染后的 `Vec<Line>` 包含带 bold 样式的 "world"

#### Scenario: 增量行返回
- **WHEN** 已提交过 3 行，buffer 中又累积了 2 行新内容并遇到 `\n`
- **THEN** `commit_complete_lines()` SHALL 只返回新增的 2 行，不重复返回已提交的行

#### Scenario: finalize 处理剩余内容
- **WHEN** 流结束时 buffer 中还有未遇到 `\n` 的剩余文本
- **THEN** `finalize()` SHALL 渲染并返回剩余内容，清空 buffer

### Requirement: StreamState 带时间戳的 FIFO 队列
`StreamState` SHALL 使用 `VecDeque<QueuedLine>` 管理待渲染的行，每行记录入队时间戳。

#### Scenario: 入队带时间戳
- **WHEN** 调用 `enqueue(lines)`
- **THEN** 每行 SHALL 以当前 `Instant::now()` 作为 `enqueued_at` 入队

#### Scenario: step 取一行
- **WHEN** 队列非空时调用 `step()`
- **THEN** SHALL 从队头弹出并返回一行

#### Scenario: drain_n 批量取
- **WHEN** 队列有 10 行时调用 `drain_n(5)`
- **THEN** SHALL 从队头弹出 5 行返回，队列剩余 5 行

#### Scenario: oldest_queued_age 查询
- **WHEN** 队列非空
- **THEN** `oldest_queued_age(now)` SHALL 返回 `now - 队头行的 enqueued_at`

### Requirement: AdaptiveChunkingPolicy 自适应背压
`AdaptiveChunkingPolicy` SHALL 实现双模式状态机，根据队列压力在 Smooth 和 CatchUp 模式间切换。

#### Scenario: 默认 Smooth 模式
- **WHEN** 策略初始化
- **THEN** SHALL 处于 Smooth 模式，每次 tick 取 1 行

#### Scenario: 进入 CatchUp
- **WHEN** 队列深度 ≥ 8 行 OR 最老行年龄 ≥ 120ms
- **THEN** SHALL 切换到 CatchUp 模式，每次 tick 取全部排队行

#### Scenario: 退出 CatchUp（迟滞）
- **WHEN** 处于 CatchUp 模式，且队列深度 ≤ 2 行 AND 最老行年龄 ≤ 40ms 持续 250ms
- **THEN** SHALL 切换回 Smooth 模式

#### Scenario: 重新进入冷却
- **WHEN** 刚从 CatchUp 退出不到 250ms
- **THEN** SHALL NOT 重新进入 CatchUp（除非触发严重积压条件）

#### Scenario: 严重积压逃逸
- **WHEN** 队列深度 ≥ 64 行 OR 最老行年龄 ≥ 300ms
- **THEN** SHALL 立即进入 CatchUp，无视重新进入冷却

#### Scenario: 空队列重置
- **WHEN** 队列为空
- **THEN** SHALL 立即重置为 Smooth 模式，清除所有迟滞状态

### Requirement: Commit Tick 编排
流式渲染期间 SHALL 以 ~60Hz 频率执行 commit tick，驱动队列 drain 和行插入。

#### Scenario: 流式开始启动 tick
- **WHEN** 收到第一个 `AgentEvent::TextDelta`
- **THEN** SHALL 启动 commit tick 动画（通过 FrameScheduler 持续请求重绘）

#### Scenario: tick 执行 drain
- **WHEN** commit tick 触发
- **THEN** SHALL 调用 `AdaptiveChunkingPolicy::decide()` 获取 drain 数量，从 StreamState 中取出对应行数，调用 `insert_lines_above()` 渲染

#### Scenario: 流结束停止 tick
- **WHEN** 收到 `AgentEvent::Done` 且队列已清空
- **THEN** SHALL 停止 commit tick 动画
