## Why

用户在 krew-cli 中的对话和输入历史在程序退出后全部丢失。用户无法恢复之前的会话上下文，也无法用上下箭头调出跨会话的输入记录。Phase 7 要求实现会话持久化，同时用户明确需要独立于会话的输入历史持久化。

## What Changes

- **会话存储实现**：`krew-storage` 的 `session_file.rs` 从 `todo!()` stub 变为完整实现，支持 TOML 格式的 `save_session()` / `load_session()` / `list_sessions()`
- **Session 序列化**：`krew-core::Session` 添加 `Serialize`/`Deserialize` derive 和构造方法
- **实时持久化**：每条消息（用户 + Agent 回复）发送/接收后实时写入 `.krew/sessions/<id>.toml`
- **会话生命周期**：App 启动时自动创建新 Session（UUID），维护 `created_at`/`updated_at`
- **/new 命令增强**：保存当前会话 → 清空上下文 → 创建新会话（当前只是清屏）
- **/resume 命令实现**：列出历史会话（按时间倒序）→ 用户选择 → 加载消息历史
- **--resume CLI 参数**：启动时直接恢复指定会话（参数已定义但未使用）
- **输入历史持久化**：新增 `.krew/history` 纯文本文件，项目级，跨会话共享，追加写入，启动时加载并截断

## Capabilities

### New Capabilities
- `session-storage`: 会话的 TOML 序列化/反序列化、文件读写、会话列表扫描
- `session-lifecycle`: App 中 Session 的创建、实时保存、/new 重建、/resume 恢复、--resume 启动恢复
- `input-history-persistence`: 输入历史的纯文本持久化，独立于会话，追加写 + 启动截断

### Modified Capabilities
- `slash-commands`: /new 从纯清屏改为保存+新建会话，/resume 从未实现变为完整实现
- `cli-args`: --resume 参数从未使用变为实际处理

## Impact

- **crates/krew-storage**：`session_file.rs` 完整重写，新增 `history_file.rs`
- **crates/krew-core**：`session.rs` 添加 serde derive + 构造方法；可能需要消息类型转换逻辑（`krew_llm::ChatMessage` ↔ `krew_core::message::ChatMessage`）
- **crates/krew-cli**：`state.rs` 集成 Session 实例；`commands.rs` 实现 /new /resume；`main.rs` 处理 --resume；`input.rs` 或 `message.rs` 追加写 history
- **文件系统**：自动创建 `.krew/sessions/` 目录和 `.krew/history` 文件
