## Context

krew-cli 的 TUI 使用 inline viewport 模型，只有输入区域 + 状态栏在 ratatui viewport 内（4 行：分隔线、输入框、分隔线、状态栏）。其他内容通过 `insert_before` 插入 viewport 上方，滚入终端 scrollback 历史。目前 agent 工作期间没有任何视觉反馈——用户看不到哪个 agent 在工作、持续多久、能否中断。

已有的基础设施：
- `commit_tick_active` / `is_thinking` / `current_agent_name` 状态字段已在跟踪 agent 活动
- `FrameRequester` 提供合并帧调度（上限 120 FPS）
- `ensure_viewport_height(needed)` 动态调整 viewport 行数（popup 已在使用）
- `popup.extra_height()` 模式展示了如何条件性增加行数

## Goals / Non-Goals

**Goals:**
- Agent 活跃处理期间，在输入区域上方显示状态指示行
- 显示 spinner 动画、agent 显示名、已用时间和中断提示
- 动态扩展 viewport（4 → 5 行），结束时恢复
- 复用已有的 commit tick / frame scheduler，保持高效

**Non-Goals:**
- Shimmer/渐变色效果（v1 简单闪烁即可）
- 计时器暂停/恢复（目前没有审批弹窗）
- 逐工具调用的状态详情（如 "Reading file..."）
- 状态行内 token 计数（回复结束后已有显示）

## Decisions

### D1: 状态行位置——上分隔线正上方

状态行作为 viewport 的第一行渲染，位于上分隔线正上方：

```
活跃态 (5 行):                  空闲态 (4 行):
  ● claude Working  (12s)      ─────────────────────
─────────────────────           › _
› _                             ─────────────────────
─────────────────────             auto · 100k · /path
  auto · 100k · /path
```

**理由**：状态行紧邻上方的流式内容，视觉连续性好。符合用户自然的视线移动方向：内容向下流 → 状态行 → 输入区域。

**考虑过的替代方案**：放在底部分隔线下方（替换状态栏）。否决原因：会遮挡 config 信息，且与流式内容在视觉上断开。

### D2: 动画方案——基于 commit tick 的闪烁

复用已有的 `commit_tick`（16ms / ~60Hz）驱动简单闪烁动画：每 600ms 在 `●`（亮态）和 `◦`（暗态）之间交替。不做 shimmer、不做渐变色。

```
第 N 帧 (0–599ms):   ● claude Working  (12s · ESC to interrupt)
第 N+1 帧 (600ms+):  ◦ claude Working  (13s · ESC to interrupt)
```

**理由**：commit tick 在流式期间已经以 60Hz 运行，搭便车即可，无需新增定时器。600ms 闪烁无需 true-color 支持，兼容所有终端。

**需要注意**：`ResponseStart` 到第一个 `TextDelta` 之间 commit tick 尚未启动。此时需通过 FrameRequester 安排周期性帧刷新来驱动 spinner 动画。

**考虑过的替代方案**：独立的动画定时器。否决原因：commit tick 的生命周期与状态行完全一致，没必要多加一个。

### D3: 状态管理——在 App 上最小化新增字段

在 `App` 上新增三个字段：
- `agent_start_time: Option<Instant>` — 在 `ResponseStart` 时设置，`Done`/`Error` 时清除
- `agent_display_name: Option<String>` — 状态行显示的 agent 名称（来自 `ResponseStart`）
- `agent_color: Option<String>` — agent 的配色（来自 `ResponseStart`）

结合已有的 `commit_tick_active`，这些字段足以决定：
- 是否显示状态行：`agent_start_time.is_some()`
- 显示什么：agent 名称来自 `agent_display_name`，耗时从 `agent_start_time` 计算
- Spinner 相位：`(elapsed.as_millis() / 600) % 2`

### D4: Viewport 高度计算

扩展已有的 `needed` 计算：

```rust
let status_line_height = if self.agent_start_time.is_some() { 1 } else { 0 };
let needed = input_lines.max(1) + 3 + status_line_height + self.popup.extra_height();
```

与 `popup.extra_height()` 模式一致。

### D5: 已用时间格式化

紧凑格式，符合行业惯例：
- `< 60s`：`"0s"`、`"45s"`
- `1–59m`：`"1m 00s"`、`"5m 23s"`
- `≥ 1h`：`"1h 05m"`（实际使用中几乎不会出现）

实现为纯函数 `fn fmt_elapsed(secs: u64) -> String`。

## Risks / Trade-offs

- **[Viewport 跳动]** → 4→5 行的切换在 agent 开始/结束时各跳一次。可以接受，因为 popup 已经有同样的行为，用户已习惯。
- **[计时精度]** → 计时器按秒级粒度显示，每次帧绘制时更新。精度取决于帧刷新频率——commit tick 期间约 60Hz（亚秒级精度），纯等待期间取决于 FrameRequester。缓解措施：状态行可见时确保 frame scheduler 保持刷新。
- **[多 Agent 计时]** → 每个 `ResponseStart` 重置计时器。在串行 reply 模式下这是正确的——每个 agent 有自己的计时器。如果未来加入并行 agent，需要重新设计。
