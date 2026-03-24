## Context

当前 krew-cli 没有全局语言配置。用户如果想让所有 Agent 用中文回复，必须在每个 Agent 的 `system_prompt` 中分别添加语言指令。这既繁琐又不一致。

Claude Code 通过 `language` 设置在 system prompt 中注入固定格式的语言指令，效果很好。krew-cli 应当采用相同的方式。

## Goals / Non-Goals

**Goals:**
- 在 `[settings]` 中添加 `language` 字段（可选，默认不配置）
- 当配置了 `language` 时，在每个 Agent 的 system prompt identity 块中自动注入语言指令；未配置时不做任何注入
- 注入文本与 Claude Code 完全一致
- 支持 user 级和 project 级配置合并

**Non-Goals:**
- 不支持 per-agent 级别的语言覆盖（用户可通过 agent 的 `system_prompt` 自行实现）
- 不做语言值的校验（任意字符串，由 LLM 自行理解）

## Decisions

### 1. 默认值为 `None`（不配置）

**选择**: `language` 字段类型为 `Option<String>`，默认 `None`。

**理由**: 未配置时不注入任何语言指令，保持 LLM 原有行为，对现有用户零影响。只有显式设置了 `language` 才会注入指令。

### 2. 注入位置：基础 identity 块中（日期时间行之后）

**选择**: 将语言指令追加到 `agent/mod.rs` 中基础 identity 字符串的末尾（日期时间行之后），位于 peer agent 协作提示和 whisper 上下文之前。

**理由**: 语言是 Agent 的身份属性（与名字、模型同级），应归入核心 identity 而非会话上下文。LLM 对 system prompt 前部的指令权重更高，放在这里比追加到整个 identity 最末更有效。同时不需要修改 `build_system_prompt` 函数签名。

### 3. 传递方式：`AgentRuntime` 新增字段

**选择**: 在 `AgentRuntime` 结构体上添加 `language: Option<String>` 字段，初始化时从 `Settings` 中获取。

**理由**: `start_completion` 方法已经通过 `self` 访问配置，无需修改方法签名。

### 4. 注入模板

固定格式，与 Claude Code 完全一致：

```
Always respond in {language}. Use {language} for all explanations, comments, and communications with the user. Technical terms and code identifiers should remain in their original form.
```

## Risks / Trade-offs

- **[风险] 与 agent `system_prompt` 中的语言指令冲突** → 全局 `language` 在 identity 块中，agent `system_prompt` 在其后。LLM 通常以后出现的指令为准，因此 agent 级别的 `system_prompt` 可以覆盖全局语言设置。这是预期行为。
