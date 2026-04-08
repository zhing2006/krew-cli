## 1. Config 层：ThinkingEffort 枚举

- [x] 1.1 在 `krew-config/src/lib.rs` 的 `ThinkingEffort` 枚举中添加 `Max` 变体
- [x] 1.2 在 `krew-config/tests/config_test.rs` 中添加 `"max"` 反序列化测试

## 2. Anthropic Client：能力矩阵与 effort 参数

- [x] 2.1 在 `krew-llm/src/anthropic.rs` 中将 `is_adaptive_model()` 替换为三个能力函数：`supports_adaptive()`（opus/sonnet + 4-6）、`supports_effort()`（opus/sonnet + 4-6, opus + 4-5）、`supports_max_effort()`（opus/sonnet + 4-6）
- [x] 2.2 修改 `build_thinking_params()` 使用 `supports_adaptive()`：adaptive 模型 → `{type: "adaptive"}`，其他 → `{type: "enabled", budget_tokens: N}`，Max 在 budget 映射中等同 High（32768）
- [x] 2.3 修改 `build_output_config()` 使用能力函数：`supports_max_effort` → effort 含 max，`supports_effort` 但非 max → effort 含 low/medium/high（Max 降为 high），其他 → None
- [x] 2.4 更新 anthropic.rs 中的现有测试，补充 Opus 4.5 effort 测试、Max 降级测试、Legacy + Max 测试

## 3. 其他 Provider：补充 Max arm

- [x] 3.1 `krew-llm/src/google.rs`：所有 `ThinkingEffort` match 分支补充 `Max` arm，等同 `High` 处理
- [x] 3.2 `krew-llm/src/openai_responses.rs`：添加 `supports_xhigh(model)` 白名单函数（gpt-5.4/gpt-5.4-pro/gpt-5.3-codex/gpt-5.2），`build_reasoning_params()` 中 Max → 白名单内 `"xhigh"`，其余 `"high"`
- [x] 3.3 `krew-llm/src/openai_chat.rs`：添加 `supports_xhigh(model)` 白名单函数，`build_reasoning_effort()` 中 Max → 白名单内 `"xhigh"`，其余 `"high"`

## 4. 帮助文本与文档

- [x] 4.1 `krew-cli/src/config_cmd/help.rs`：thinking_effort 可选值更新为 `"low" | "medium" | "high" | "max"`
- [x] 4.2 `docs/MANUAL.md`：thinking_effort 注释更新
- [x] 4.3 `docs/MANUAL_CN.md`：thinking_effort 注释更新
- [x] 4.4 `docs/TDD.md`：ThinkingEffort 枚举（加 Max）、Anthropic 能力矩阵、OpenAI supports_xhigh 白名单及降级规则更新

## 5. 验证

- [x] 5.1 运行 `cargo test` 确保所有测试通过
- [x] 5.2 运行 `cargo clippy --all-targets --all-features -- -D warnings` 确保无警告
