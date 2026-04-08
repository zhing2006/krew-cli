## Why

Anthropic 在 Claude 4.6 系列引入了 adaptive thinking 和 `effort: "max"` 级别，但当前实现缺少 `Max` effort 支持，且 Opus 4.5 等中间代模型虽然支持 `effort` 参数却被跳过。需要按模型分层正确应用 thinking 和 effort 参数，避免在不支持的模型上发送会导致 API 错误的参数。

## What Changes

- `ThinkingEffort` 枚举新增 `Max` 变体，支持 TOML `"max"` 反序列化
- Anthropic client 引入能力函数矩阵，按模型名精确判断各能力的支持情况：
  - adaptive thinking：Opus 4.6, Sonnet 4.6
  - effort 参数：Opus 4.6, Sonnet 4.6, Opus 4.5
  - max effort：Opus 4.6, Sonnet 4.6
  - 不支持 max 时静默降为 high，不支持 effort 时不发送 output_config
- Google 的 `ThinkingEffort::Max` 等同 `High` 处理
- OpenAI 按模型白名单判断：已知支持 xhigh 的模型发 `"xhigh"`，其余降为 `"high"`
- 帮助文本、双语 MANUAL 文档和 TDD 文档更新

## Capabilities

### New Capabilities

（无新增 capability）

### Modified Capabilities

- `thinking-config`: 新增 `Max` effort 级别，Anthropic 按能力矩阵精确控制 effort 发送范围
- `config-types`: `ThinkingEffort` 枚举新增 `Max` 变体

## Impact

- **krew-config**: `ThinkingEffort` 枚举定义变更，所有 `match` 分支需补全
- **krew-llm**: anthropic.rs 能力矩阵重构，google.rs Max 等同 High，openai_responses.rs / openai_chat.rs 加 `supports_xhigh` 白名单判断
- **krew-cli**: config help 文本更新
- **docs**: MANUAL.md / MANUAL_CN.md / TDD.md thinking_effort 可选值更新
