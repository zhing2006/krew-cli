# Phase 11: Compact + Token 管理

> 目标：实现 `/compact` 手动压缩和自动压缩，完善 Token 追踪。

## 实现内容

- **/compact 命令**：指定 Agent 将历史消息压缩为摘要
  - 保留最后 N 条消息，压缩其余
  - 压缩前备份到 `.pre-compact.{timestamp}.toml`
  - 压缩结果作为 System 消息注入
- **自动压缩**：当 `prompt_tokens >= auto_compact_threshold` 时，下次对话前自动触发
  - 使用 `reply_order` 第一个 Agent 执行
  - 显示提示 `⚡ 会话已自动压缩 (N tokens → M tokens)`
- **/agents 增强**：显示每个 Agent 的累计 token 用量
- **Token 追踪完善**：`session.total_tokens_used` 精确累计

## 验收标准

```txt
you> /compact opus
⚡ 会话已压缩 (45,000 tokens → 3,200 tokens)
备份: .krew/sessions/a1b2c3d4.pre-compact.1709136000.toml

you> /agents
Agents in session:
  [gpt]  GPT-5.2      openai/gpt-5.2           3,284 tokens (1,250 in / 2,034 out)
  [opus] Claude Opus   anthropic/claude-opus-4-6 5,642 tokens (3,512 in / 2,130 out)
──────────────────────────────────────────────────────
  Total: 8,926 tokens
```

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L196-197 | §4.3 /compact 命令定义 |
| PDD | L255-261 | §4.5.3 自动压缩说明 |
| TDD | L525-536 | §3.5.1 /agents 输出规格 |
| TDD | L543-561 | §3.5.2 /compact 实现方案（流程 + 备份） |
| TDD | L563-616 | §3.5.3 自动压缩（触发条件、流程、配置） |
| TDD | L573-583 | §3.5.3 各 Provider Usage 返回方式映射表 |
