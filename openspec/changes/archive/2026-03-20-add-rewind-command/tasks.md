## 1. SlashCommand 注册

- [x] 1.1 在 `krew-core/src/command.rs` 的 `SlashCommand` enum 中添加 `Rewind` variant，更新 `from_input()`（匹配 `/rewind`）、`name()`、`all_help()`（描述 "Rewind to a previous message"）、`description()` 方法
- [x] 1.2 在 `krew-cli/src/completion.rs` 中将 `/rewind` 添加到 tab 补全候选列表

## 2. RewindPicker Popup

- [x] 2.1 在 `krew-cli/src/completion.rs` 的 `ActivePopup` enum 中添加 `RewindPicker(CompletionState)` variant，更新 `extra_height()` 等方法（与 SessionPicker 同逻辑）
- [x] 2.2 在 `krew-cli/src/app/input.rs` 中添加 RewindPicker 的键盘事件处理逻辑（上下键浏览、Enter 确认、ESC 取消），确认时解析 item.value 为原始 messages 下标，调用 `apply_rewind()`

## 3. App 状态扩展

- [x] 3.1 在 `krew-cli/src/app/state.rs` 的 `App` struct 中添加 `rewound: bool` 字段，在 `App::new()` 中初始化为 `false`
- [x] 3.2 在 `krew-cli/src/app/message.rs` 的 `send_message()` 和 `send_expanded_text()` 方法入口处添加 rewound 检查：若 `self.rewound` 为 true，生成新 `session_id` 和 `session_created_at`，设置 `rewound = false`

## 4. Rewind 核心逻辑

- [x] 4.1 在 `krew-cli/src/app/commands.rs` 中添加 `execute_rewind()` 方法：收集所有 role=User 的消息及其在 `self.messages` 中的原始下标，按时间正序构建 CompletionItem 列表（value = 原始下标字符串，description = `时间  "内容预览"`），默认选中最后一条，弹出 RewindPicker popup；若无用户消息则显示 info 提示
- [x] 4.2 在 `krew-cli/src/app/commands.rs` 中添加 `apply_rewind()` 方法：接受原始 messages 下标，执行 `self.messages.truncate(index)`；从截断后的 messages 重建 `agent_token_usage`、`last_respondent`、skill activation state；设置 `rewound = true`
- [x] 4.3 在 `apply_rewind()` 中处理边界情况：若选中的用户消息下标为 0，直接调用 `execute_new()`（完全等同 /clear），不截断、不设 rewound，直接 return

## 5. 消息重放提取

- [x] 5.1 从 `load_session()` 方法中提取消息重放逻辑为独立方法 `replay_messages(&[MessageEntry])`，使 `load_session()` 调用此方法
- [x] 5.2 在 `apply_rewind()` 中使用 `build_session_file()` 将截断后的 `Vec<ChatMessage>` 转为 `SessionFile`，然后调用 `replay_messages()` 进行清屏重放

## 6. 保存行为与状态转移

- [x] 6.1 在 `krew-cli/src/app/persistence.rs` 的 `save_session()` 方法开头添加统一守卫：若 `self.rewound` 为 true 则直接 return（需将 `&self` 改为 `&mut self` 或使用其他方式读取 rewound 标记。注意：save_session 当前签名是 `&self`，rewound 是 bool 可直接读取无需 &mut）
- [x] 6.2 在 `krew-cli/src/app/input.rs` 的 SessionPicker Enter 处理中，`load_session()` 成功后添加 `self.rewound = false`
- [x] 6.3 在 `krew-cli/src/app/commands.rs` 的 `execute_new()` 中添加 `self.rewound = false` 和 `self.session_created_at = Utc::now()`（在生成新 session_id 的同时重置创建时间）
- [x] 6.4 在 `krew-cli/src/app/commands.rs` 的 `execute_compact()` 中添加 rewound 检查：若 `self.rewound` 为 true 则显示提示信息拒绝执行（因为 compact 有独立的 backup 写盘路径绕过 `save_session()` 守卫）

## 7. 验证

- [x] 7.1 `cargo fmt --all` 和 `cargo clippy --all-targets --all-features -- -D warnings` 通过
- [x] 7.2 `cargo test` 全部通过
- [x] 7.3 手动验证基本流程：多轮对话后执行 `/rewind`，确认 popup 列表按时间正序正确显示所有用户消息且默认选中最后一条、选择后对话正确截断和重放、原始 session 文件未被修改、发新消息后生成新 session ID
- [x] 7.4 手动验证 rewind → /resume：确认原始 session 文件未被修改，加载的新 session 不继承 rewound 状态
- [x] 7.5 手动验证 rewind → /compact：确认被拒绝执行并显示提示信息
- [x] 7.6 手动验证 rewind → 选第一条用户消息：确认完全等同 /clear（原始 session 被完整保存，生成新 session ID）
- [x] 7.8 手动验证 rewind → /exit：确认原始 session 文件未被截断内容覆盖
- [x] 7.7 手动验证 popup 列表正序显示且默认选中末尾项，选择后能正确映射到原始 messages 下标进行截断
