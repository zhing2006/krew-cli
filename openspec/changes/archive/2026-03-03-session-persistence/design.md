## Context

krew-cli Phase 6 已完成多 Agent 协作，但所有对话和输入历史在程序退出后丢失。当前状态：

- `krew-storage::session_file` 三个函数均为 `todo!()` stub
- `krew-core::Session` 已定义但无 Serialize/Deserialize，无构造方法
- App 运行时使用 `krew_llm::ChatMessage`（简化版：role + content + name），与持久化用的 `krew_core::message::ChatMessage`（完整版：含 addressee, tool_calls, usage, created_at 等）是不同类型
- `/new` 只是清屏，`/resume` 和 `--resume` 均未实现
- 输入历史 `App.history: Vec<String>` 纯内存，无持久化

## Goals / Non-Goals

**Goals:**
- 会话自动持久化到 `.krew/sessions/<id>.toml`，每条消息实时写入
- `/new` 保存当前会话并开始新会话，`/resume` 列出并恢复历史会话
- `--resume` CLI 参数启动时恢复指定会话
- 输入历史独立持久化到 `.krew/history`，跨会话共享
- 启动时显示会话 ID

**Non-Goals:**
- 工具调用（tool_calls/tool_results）的持久化 —— Phase 8 工具系统尚未实现，当前消息不含此信息
- 会话搜索/过滤功能
- 会话删除/清理命令
- 跨项目的全局会话存储

## Decisions

### D1: 会话文件格式遵循 TDD §3.6.1

按现有 TDD 规范使用 TOML 格式。`[session]` 表存元数据，`[[messages]]` 数组存消息历史。

**替代方案**：JSON Lines（每条消息一行追加）—— 追加效率高但 TOML 已是项目约定格式，且每次全量重写对会话长度可接受。

### D2: 全量重写而非追加

每次保存时将整个 Session 序列化为 TOML 并覆盖写入文件。

**理由**：TOML 格式不适合追加（`[session]` 元数据如 `updated_at`、`total_tokens_used` 每次都变），且会话消息量级有限（compact 前通常 <100 条）。原子写（写临时文件 + rename）保证崩溃安全。

**替代方案**：SQLite —— 过度设计，引入新依赖。

### D3: 消息类型简化 —— 不引入双类型转换

当前 `krew_llm::ChatMessage` 是运行时唯一使用的消息类型。为了避免引入复杂的双向转换逻辑，**持久化时直接使用一个专用的 TOML 序列化结构体 `SessionFile`**，在 `krew-storage` 中定义，负责 `Session` ↔ TOML 的转换。

App 中的 `messages: Vec<krew_llm::ChatMessage>` 在保存时映射为 TOML 消息格式，恢复时反向映射回 `krew_llm::ChatMessage`。这避免了修改 `krew_llm::ChatMessage` 的 serde derive 或在 App 中使用两套消息类型。

### D4: Session 生命周期由 App 管理

- `App::new()` 中生成 UUID 并创建 `Session`，但 App 本身不存储 `Session` 实例，只存储 `session_id: String` 和 `session_dir: PathBuf`
- 保存时从 App 的运行时状态（messages, agent_token_usage, agents 等）构建 `SessionFile` 并写入
- 保存时机：每次用户消息发送后 + 每次 agent 回复完成后

### D5: 输入历史使用纯文本文件

- 文件路径：`.krew/history`
- 格式：一行一条，多行输入中 `\n` 转义为 `\\n`，`\\` 转义为 `\\\\`
- 运行时追加写：每次 `history_push()` 时 `OpenOptions::append` 写入一行
- 启动加载：读取全部行，取最后 `input_history_limit` 条填充 `App.history`，然后用截断后的内容重写文件
- 与会话完全独立，`/new` 不影响历史

### D6: /resume 使用序号选择而非 ID

`/resume` 列出最近 N 个会话（按 `updated_at` 倒序），每个显示序号 + 时间 + agents + 首条消息摘要。用户输入序号选择。

实现方式：`/resume` 触发后进入一个特殊的选择模式（在 info 区域显示列表，等待用户输入数字）。由于当前 TUI 架构是单行输入+滚动输出，最简方案是将列表显示在输出区域，用户通过输入数字并回车来选择。

### D7: 原子写保证文件完整性

写入会话文件时：先写入 `<id>.toml.tmp`，成功后 rename 为 `<id>.toml`。防止写入中途崩溃导致文件损坏。

## Risks / Trade-offs

- **[全量重写性能]** → 当前会话消息量级小（<100 条），TOML 序列化+写入 <10ms，可接受。当 compact 功能实现后消息量会进一步控制
- **[并发写入]** → 单进程单线程写入，无并发风险
- **[磁盘空间]** → 未实现会话清理，长期使用会积累文件 → 可在后续 phase 添加自动清理
- **[/resume 选择体验]** → 纯文本序号选择不如交互式列表优雅，但实现简单且符合现有 TUI 架构。后续可升级为箭头键选择
