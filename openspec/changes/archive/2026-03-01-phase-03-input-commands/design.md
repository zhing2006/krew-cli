## Context

Phase 1-2 已完成 TUI 框架（ratatui inline viewport + 自定义 Terminal）和配置系统（TOML 加载 + AGENTS.md）。当前用户输入只有硬编码的 echo 回显和 `/quit` 退出，`parse_input()` 和 `SlashCommand` 枚举已存在但未接入 TUI 流程。

现有代码基础：
- `krew-core::router::parse_input()` — 解析 @all/@name/无前缀，返回 `(Addressee, String)`
- `krew-core::command::SlashCommand` — 枚举 + `from_input()` 解析 + `name()`/`description()` 元数据
- `krew-cli::app::App::send_message()` — 硬编码 `/quit` 检查 + echo 回显
- `krew-cli::custom_terminal::Terminal::ensure_viewport_height()` — 动态 viewport 高度（可滚动推上方内容）

## Goals / Non-Goals

**Goals:**
- `@` 寻址支持任意位置识别（开头、中间、结尾均可），只匹配已知 Agent 名称
- 未知 `@token` 和裸 `@` 静默当作普通文本（不报错）
- 消息正文保留完整原文（含 `@name`），不剥离
- Slash 命令识别并执行（/help /agents /clear /quit 完整实现，/new /resume /compact 占位）
- 补全弹窗在输入 `/` 或 `@` 时自动触发，替换状态栏区域
- 用户消息带彩色圆点指示目标 Agent，Echo 用黄色菱形前缀

**Non-Goals:**
- LLM 实际调用（Phase 4）
- 会话持久化（Phase 7）
- 输入历史（上下箭头浏览）— 可后续单独添加
- @ Agent 名称的模糊匹配 — 仅做精确前缀匹配

## Decisions

### D1: 命令执行逻辑放在 krew-cli（方案 B）

**决定**: `SlashCommand` 在 `krew-core` 中保持为纯数据枚举（解析 + 元数据），执行逻辑在 `krew-cli::app.rs` 中通过 match 分发。

**理由**: `/clear` 需要 terminal 引用、`/agents` 需要 config + TUI 渲染，这些都是 TUI 层的关注点。如果在 `krew-core` 中实现 `execute()`，需要定义 `AppContext` trait 抽象 TUI 操作，过度工程化。

**替代方案**: 在 `krew-core` 定义 `AppContext` trait 并实现 `SlashCommand::execute()` — 增加了 trait 抽象但命令只有 7 个，不值得。

### D2: Agent 校验内置于 parse_input

**决定**: `parse_input(input, known_agents)` 接受已知 Agent 列表，只有匹配的 `@name` 才被识别为寻址目标。未知的 `@token` 静默当作普通文本。独立的 `validate_addressee()` 已移除。

**理由**: 将校验融入解析更自然——未知 `@name` 不应报错（用户可能在讨论中提到 `@someone`），而且消息正文需要保留完整原文以便 LLM 理解上下文。

**替代方案**: 之前方案是 `parse_input()` 纯解析 + `validate_addressee()` 独立校验并报错。实际使用中发现未知 `@` 报错体验不好，改为静默处理。

### D3: 补全弹窗替换状态栏区域

**决定**: 弹窗激活时，viewport 底部区域从「分隔线 + 状态栏」（2 行）变为「分隔线 + 补全列表」（2+N 行）。利用已有的 `ensure_viewport_height()` 自动扩展 viewport 并推上方内容。

**布局变化**:
```
正常 (height=4):          弹窗激活 (height=4+N):
├──────────────┤          ├──────────────┤
│ › 输入区域   │          │ › /ag█       │
├──────────────┤          ├──────────────┤
│ 状态栏       │          │  /agents  …  │  ← 弹窗替换状态栏
└──────────────┘          │  /clear   …  │     并向下扩展
                          │  /help    …  │
                          └──────────────┘
```

**理由**: 与 codex 的行为完全一致。codex 的 `desired_height()` 在弹窗激活时增加弹窗高度，`tui.draw(height)` 触发 `scroll_region_up` 推上方内容。我们的 `ensure_viewport_height()` 做的事情一模一样。

### D4: 弹窗状态管理

**决定**: 在 `App` 中添加 `ActivePopup` 枚举，互斥管理弹窗状态：

```rust
enum ActivePopup {
    None,
    SlashCommand(CompletionState),
    AgentName(CompletionState),
}

struct CompletionState {
    filter: String,        // 当前过滤文本
    selected: usize,       // 选中项索引
    items: Vec<String>,    // 过滤后的匹配项
}
```

每次按键后调用 `sync_popup()` 检测触发条件，自动开启/关闭弹窗。

### D5: 弹窗键盘交互

**决定**: 弹窗激活时拦截特定按键：
- **↑/↓** — 导航选中项（回绕）
- **Tab/Enter** — 确认选中项，插入到输入框
- **Esc** — 关闭弹窗
- **其他键** — 继续输入，更新过滤

### D6: /agents 输出格式

**决定**: 按 TDD §3.5.1 格式输出，但 token 统计占位为 0：
```
Agents:
  [gpt]  GPT-5.2      openai/gpt-5.2           0 tokens
  [opus] Claude Opus   anthropic/claude-opus-4-6 0 tokens
```

## Risks / Trade-offs

- **[Risk] ratatui-textarea 不支持自定义补全** → 自行实现补全弹窗 UI，不依赖 textarea 的补全功能。弹窗是独立的渲染区域，与 textarea 无关。
- **[Risk] 弹窗扩展 viewport 可能闪烁** → `ensure_viewport_height()` 已经过 Phase 1 验证，多行输入扩展时无闪烁。弹窗扩展原理相同。
- **[Trade-off] 补全只做前缀匹配** → 命令和 Agent 名称数量少（7 + N），前缀匹配已足够。模糊匹配增加复杂度但收益低。
