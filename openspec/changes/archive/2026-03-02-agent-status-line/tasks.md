## 1. 状态字段与工具函数

- [x] 1.1 在 `App` 结构体中新增 `agent_start_time: Option<Instant>`、`agent_display_name: Option<String>`、`agent_color: Option<String>` 字段，并在 `App::new()` 中初始化为 `None`
- [x] 1.2 在 `viewport.rs` 中实现 `fn fmt_elapsed(secs: u64) -> String` 紧凑时间格式化函数（`0s`、`45s`、`1m 23s`、`1h 05m`）

## 2. 事件驱动生命周期

- [x] 2.1 在 `handle_agent_event` 的 `ResponseStart` 分支中设置 `agent_start_time = Some(Instant::now())`、`agent_display_name` 和 `agent_color`
- [x] 2.2 在 `handle_agent_event` 的 `Done` 和 `Error` 分支中清除 `agent_start_time`、`agent_display_name`、`agent_color`（设为 `None`）

## 3. 状态行渲染

- [x] 3.1 在 `viewport.rs` 中实现 `fn render_agent_status(frame, app, area)` 渲染函数：根据 `agent_start_time` 计算 elapsed，根据 600ms 周期决定 spinner 符号（`●`/`◦`），组合 agent 名称、"Working"、时间、中断提示的 `Span` 列表
- [x] 3.2 在 `render_input_viewport` 中修改布局逻辑：当 `app.agent_start_time.is_some()` 时在 `Layout::vertical` 约束列表头部插入 `Constraint::Length(1)` 并调用 `render_agent_status`

## 4. Viewport 高度动态调整

- [x] 4.1 在 `state.rs` 的 draw frame 分支中修改 `needed` 计算：加入 `status_line_height`（`agent_start_time.is_some()` 时为 1，否则为 0）
- [x] 4.2 确保状态行可见期间 FrameRequester 保持周期性帧刷新（在 `ResponseStart` 后 commit tick 启动前，通过 `schedule_frame_in` 安排延迟帧）
