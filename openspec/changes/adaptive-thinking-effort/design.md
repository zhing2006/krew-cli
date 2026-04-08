## Context

当前 Anthropic client 使用 `is_adaptive_model()` 二分法判断模型：要么 adaptive（4.6），要么 legacy（其他）。这遗漏了 Opus 4.5 等支持 `effort` 参数但不支持 adaptive thinking 的中间代模型。同时 `ThinkingEffort` 枚举缺少 `Max` 变体，无法利用 4.6 模型的最高推理能力。

现有代码中 `build_output_config()` 在非 adaptive 模型上直接返回 `None`，导致 Opus 4.5 完全不发送 effort 参数。

Anthropic 官方的能力分布并非按代际整齐划分：
- adaptive thinking：仅 Opus 4.6, Sonnet 4.6
- effort 参数：Opus 4.6, Sonnet 4.6, Opus 4.5（Sonnet 4.5 / Haiku 4.5 未在官方列表中）
- max effort：仅 Opus 4.6, Sonnet 4.6

## Goals / Non-Goals

**Goals:**
- 在 `ThinkingEffort` 枚举中添加 `Max` 变体
- Anthropic client 采用能力函数矩阵，按模型名精确判断 adaptive / effort / max 三项能力的支持情况
- 在不支持 `Max` 的模型上静默降级为 `High`
- Google 的 `Max` 等同 `High` 处理
- OpenAI 的 `Max` 映射为 `"xhigh"`
- 更新帮助文本、双语文档和 TDD

**Non-Goals:**
- 不支持 Mythos Preview 模型（未对公众开放）
- 不将 effort 从 thinking 配置中解耦（保持绑定）
- 不为降级行为添加 tracing::warn

## Decisions

### Decision 1: 能力函数矩阵而非分层枚举

**选择**：使用独立的能力判断函数替代 `is_adaptive_model()` 布尔函数：

```rust
fn supports_adaptive(model: &str) -> bool    // opus/sonnet + 4-6
fn supports_effort(model: &str) -> bool      // opus/sonnet + 4-6, opus + 4-5
fn supports_max_effort(model: &str) -> bool  // opus/sonnet + 4-6
```

**理由**：Anthropic 的能力并非按代际整齐划分——Sonnet 4.5 / Haiku 4.5 不支持 effort 但 Opus 4.5 支持。分层枚举（Tier46/Tier45/Legacy）会错误地把同代不同能力的模型归到同一层。能力函数矩阵让每项能力独立判断，精确匹配官方支持列表。

**替代方案**：三层或四层枚举——需要为每个特殊组合添加新层级，层级会随模型增多而膨胀。

### Decision 2: Max 静默降级

**选择**：在不支持 `max` 的模型上静默降级为 `high`，不输出 warning。

**理由**：用户配置 `thinking_effort = "max"` 的意图是"尽可能强"。在不支持 max 的模型上降为最高可用级别完全符合这个意图，无需打扰用户。

### Decision 3: 模型名匹配规则

**选择**：
- `supports_adaptive`：模型名包含 `(opus|sonnet)` 且包含 `4-6`
- `supports_effort`：满足 `supports_adaptive`，或模型名包含 `opus` 且包含 `4-5`
- `supports_max_effort`：等同 `supports_adaptive`

**理由**：沿用现有字符串匹配风格，与 `default_max_tokens()` 保持一致。涵盖带日期后缀的模型 ID（如 `claude-opus-4-6-20250801`）。仅 Opus 4.5 被纳入 effort 支持，Sonnet 4.5 / Haiku 4.5 按官方文档排除。

### Decision 4: OpenAI Max 能力判断 + 降级

**选择**：OpenAI Responses 和 OpenAI Chat 中引入 `supports_xhigh(model)` 能力判断函数。已知支持 xhigh 的模型发送 `"xhigh"`，未知模型降为 `"high"`。

**匹配规则**：白名单方式，仅认出已确认支持 xhigh 的模型，其余一律降为 high。

截至 2026-04-08 已确认支持 xhigh 的模型：`gpt-5.4`、`gpt-5.4-pro`、`gpt-5.3-codex`、`gpt-5.2`。新模型出现时只需往白名单追加条目。

**理由**：OpenAI 的 reasoning effort 是 model-dependent，白名单最保守最安全——只发已知能接受的值，未知模型不冒险。与 Anthropic 侧的处理风格保持一致。

**替代方案**：
- 统一降为 high —— 限制了已支持模型的最高能力
- 统一发 xhigh —— 对旧模型会触发 API 错误

### Decision 5: Google Max 等同 High

**选择**：Google（Gemini）中 `ThinkingEffort::Max` 等同 `High` 处理。

**理由**：Gemini 的 thinking 使用 `thinkingLevel` 枚举（最高 high）或 `thinkingBudget` 数值，没有超过 high 的级别。映射到最高可用级别是合理的降级。

## Risks / Trade-offs

- **模型名匹配脆弱性** → Anthropic 如果改变命名规则可能导致误分类。缓解：字符串匹配已是既有模式，且 Anthropic 命名一直很规律。
- **Opus 4.5 effort 独占** → 仅 Opus 4.5 纳入 effort 支持，Sonnet 4.5 / Haiku 4.5 排除。如果后续 Anthropic 扩展支持，需要更新 `supports_effort()`。缓解：函数式设计让添加新模型只需修改一处。
- **OpenAI xhigh 模型列表维护** → 需要手动维护 `supports_xhigh()` 中的模型列表。缓解：默认保守（降为 high），新模型支持 xhigh 时只需添加一个匹配条件。
