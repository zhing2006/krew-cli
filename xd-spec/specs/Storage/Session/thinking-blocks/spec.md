# thinking-blocks Specification

## Purpose
TBD - created by archiving a change. Update Purpose after archive.

## Requirements
### Requirement: MessageEntry 必须能持久化 thinking blocks

`MessageEntry` MUST 暴露一个可选 `thinking_blocks: Option<Vec<ThinkingBlockEntry>>` 字段，序列化时遵循 `skip_serializing_if = "Option::is_none"`，反序列化时缺字段默认 `None`，从而旧的 session TOML 不需要迁移即可继续加载。

#### Scenario: 旧 session 文件缺字段时仍能加载

- **WHEN** 读取一份没有 `thinking_blocks` 键的旧 session TOML
- **THEN** `load_session` SHALL 成功返回，对应 `MessageEntry.thinking_blocks` SHALL 等于 `None`，且其他字段值 SHALL 不受影响

#### Scenario: 写入带 thinking blocks 的会话再读取应等值

- **WHEN** 调用 `save_session` 写入一条带两个 thinking block（一个 `Thinking{text, signature}`、一个 `Redacted{data}`）的 assistant 消息，随后用 `load_session` 重新加载
- **THEN** 加载后的 `MessageEntry.thinking_blocks` SHALL 是 `Some(vec)`，长度为 2，按原顺序保留两个 block 的类型与字段值

### Requirement: ThinkingBlockEntry 必须能区分 Thinking 与 Redacted

`ThinkingBlockEntry` 序列化形态 MUST 能明确区分 `Thinking { text, signature }` 与 `Redacted { data }` 两种变体（建议用 TOML 友好的 `block_type = "thinking" | "redacted_thinking"` 字段加按需可选字段，具体 schema 由实现决定，但必须支持往返）。

#### Scenario: Redacted 变体写入后不会出现 text/signature 字段

- **WHEN** 序列化一个仅含 `Redacted{data: "X"}` 的 thinking block 集合
- **THEN** 输出 TOML 中对应条目 MUST NOT 出现 `text` 或 `signature` 键，并 MUST 出现某种与 `Thinking` 变体可区分的标记字段

### Requirement: persistence 模块必须在 ChatMessage 与 MessageEntry 之间往返 thinking blocks

`krew-core/src/persistence.rs` 中的 ChatMessage → MessageEntry 与 MessageEntry → ChatMessage 转换 MUST 在两个方向上都保留 thinking blocks 的完整内容；空集合两侧 MUST 互相等价（持久化为缺字段 / `None`，而不是空数组）。

#### Scenario: 持久化往返不丢失 thinking blocks

- **WHEN** 把一条带 thinking_blocks 的 assistant `ChatMessage` 转成 `MessageEntry`，写盘后再加载并转回 `ChatMessage`
- **THEN** 回程得到的 `ChatMessage.thinking_blocks` SHALL 与原始内容逐元素相等

#### Scenario: 空 thinking_blocks 不产生冗余 TOML 字段

- **WHEN** 持久化一条 `thinking_blocks` 为空向量的 assistant `ChatMessage`
- **THEN** 生成的 TOML MUST NOT 出现 `thinking_blocks` 键（避免污染历史 session 文件大小与可读性）
