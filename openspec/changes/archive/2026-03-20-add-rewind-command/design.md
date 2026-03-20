## Context

krew-cli 的对话历史存储在 `App.messages: Vec<ChatMessage>` 中，每轮对话包含 User 消息和随后的 Assistant/Tool 消息。Session 通过 TOML 文件持久化在 `.krew/sessions/` 目录下。目前用户无法在不丢失整个会话的情况下回退到之前的对话节点。

现有的 `/resume` 命令已实现了完整的 session picker popup（`ActivePopup::SessionPicker`）和消息重放逻辑（`load_session()`），可以作为 `/rewind` 的参考模式。

## Goals / Non-Goals

**Goals:**
- 用户可以通过 `/rewind` 从所有用户消息中选择一个回退点
- 回退到非第一条用户消息时进入 fork 语义：原始 session 文件保持不变，发新消息时自动生成新 session ID
- 回退到第一条用户消息时完全等同 `/clear`：正常保存完整 session 后开始新会话

**Non-Goals:**
- 不支持回退到 assistant/tool 消息（仅以 user 消息为节点）
- 不支持跨 session 的 rewind（只能在当前活跃 session 内操作）
- 不做 undo/redo 机制（rewind 是单向的截断操作）

## Decisions

### Decision 1: Rewind popup 复用 completion popup 模式

**选择**: 新增 `ActivePopup::RewindPicker(CompletionState)` variant，复用现有的 `CompletionState` 选择器组件。

**理由**: `/resume` 已经用这个模式实现了 session 选择。RewindPicker 的交互逻辑（上下键选择、Enter 确认、ESC 取消）完全一致，只是数据源不同。

**替代方案**: 实现独立的选择 UI → 增加代码量且交互不一致。

### Decision 2: 消息重放提取为可复用方法

**选择**: 从 `load_session()` 中提取消息重放逻辑为独立方法 `replay_messages()`，接受 `&[MessageEntry]` 参数。`load_session()` 和 rewind 都调用此方法。

**理由**: `load_session()` 的重放逻辑（commands.rs:426-565）是基于 `SessionFile.messages`（即 `Vec<MessageEntry>`）的。Rewind 后需要将内存中的 `Vec<ChatMessage>` 转为 `Vec<MessageEntry>` 再重放——通过 `build_session_file()` 即可完成此转换，这样两个场景共享同一套重放代码。

### Decision 3: Fork 语义——统一保存守卫 + 延迟换 session ID

**选择**: 在 App 中添加 `rewound: bool` 字段。Rewind 操作只截断内存中的 messages 并设置 `rewound = true`。保存守卫放在 `save_session()` 方法内部：当 `rewound` 为 true 时直接 return，跳过所有写盘操作。在 `send_message()` / `send_expanded_text()` 入口处检查此标记，若为 true 则生成新 session_id 和 session_created_at，清除标记，后续 `save_session()` 正常写入新文件。

**理由**: `save_session()` 是唯一的写盘入口（`persistence.rs`），在此处统一守卫可以一劳永逸地覆盖所有调用点（`/exit`、`/clear`、`/resume`、`/compact`、agent Done、agent Error、ESC cancel），无需逐个修改。相比在每个调用点分别加检查，这种方案更健壮，不会因遗漏调用点而意外覆盖原始 session 文件。

**替代方案**: 在每个 `save_session()` 调用点加 `if !self.rewound` 检查 → 当前至少有 7 个调用点，容易遗漏，维护成本高。

**状态转移规则**:

| 操作 | rewound 状态变化 | 说明 |
|------|------------------|------|
| `/rewind` | → `true` | 截断对话，不保存 |
| 发新消息 | `true` → `false`，换 session_id | fork 分支点 |
| `/resume` 成功加载 | `true` → `false` | 加载的新 session 不应继承 rewound 状态 |
| `/clear` | `true` → `false` | 开始全新会话，不保存截断内容 |
| `/compact` | rewound 时直接拒绝执行 | compact 有独立的 backup 写盘路径（`create_backup()` 直接调 `krew_storage::save_session()`），不经过 `App::save_session()` 守卫，无法通过统一守卫阻止。因此 rewound 状态下禁止 compact |
| `/exit` | rewound 时 save_session() 静默跳过 | 原始 session 文件不变 |
| agent Done/Error/Cancel | rewound 不可能为 true | rewound 后用户必须先发新消息才触发 agent，此时 rewound 已被清除 |

### Decision 4: Rewind 到第一条用户消息 = 完全等同 /clear

**选择**: 如果用户选择了第一条用户消息（下标为 0），直接调用 `execute_new()`，不做任何截断、不设 rewound。此时 `execute_new()` 会正常保存当前完整的（未截断的）session，然后清屏、换 session_id、开始新会话。

**理由**: 选第一条消息意味着"丢掉整个对话重来"，这和 `/clear` 的语义完全一致。不需要 fork 语义，因为原始 session 在截断之前就已被完整保存。无需抽取 helper 或特殊处理 rewound 状态。

**附带修复**: 当前 `execute_new()` 只重置 `session_id`，不重置 `session_created_at`。需要补上 `self.session_created_at = Utc::now()`，让新会话的创建时间正确。这不仅服务 rewind 场景，也修复普通 `/clear` 的语义。

### Decision 5: Token usage 和 last_respondent 重建

**选择**: 从截断后的 messages 重新推导 `agent_token_usage` 和 `last_respondent`，逻辑与 `load_session_from_disk()` 一致。

**理由**: 简单可靠。截断后的 messages 可能移除了某些 agent 的最新回复，直接从现有数据重建比增量调整更安全。

## Risks / Trade-offs

- **[风险] Rewind 后各种路径覆盖原始 session** → 保存守卫统一放在 `save_session()` 内部，rewound 状态下静默跳过写盘
- **[风险] /resume 后 rewound 状态泄漏** → `load_session()` 成功后显式清除 `rewound = false`，避免新加载的 session 继承 fork 状态
- **[风险] Session-scoped tool state（如 skill activation）未重置** → Rewind 后从截断后的 messages 重新推导 activated_skills，与 resume 逻辑一致
- **[风险] /compact 绕过统一保存守卫** → compact 有独立的 `create_backup()` 写盘路径，不经过 `App::save_session()`。解决方案：rewound 状态下禁止 /compact 执行
- **[取舍] Popup 列表显示全部用户消息** → 对话非常长时列表会很长，但 popup 本身支持滚动，不影响可用性
