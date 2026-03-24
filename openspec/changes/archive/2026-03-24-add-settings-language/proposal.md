## Why

用户需要一个集中式的方式来指定所有 Agent 回复时使用的语言。目前唯一的办法是在每个 Agent 的 `system_prompt` 中手动添加语言指令，既重复又容易遗漏。一个全局的 `settings.language` 配置项可以提供统一的控制点，与 Claude Code 的做法一致。

## What Changes

- 在 `[settings]` 中新增 `language` 字段（可选字符串，默认不配置）
- 当该字段有值时，在每个 Agent 的 system prompt identity 块中注入语言指令；未配置时不做任何注入
- 注入文本与 Claude Code 完全一致：`"Always respond in {language}. Use {language} for all explanations, comments, and communications with the user. Technical terms and code identifiers should remain in their original form."`
- 支持 user 级（`~/.krew/settings.toml`）和 project 级（`.krew/settings.toml`）配置，遵循标准合并语义（project 覆盖 user）

## Capabilities

### New Capabilities

- `language-setting`: 全局语言配置，包括配置解析、合并，以及 system prompt 注入

### Modified Capabilities

## Impact

- `krew-config`: `Settings`、`RawSettings`、`UserSettings` 结构体新增 `language` 字段；合并和解析逻辑更新
- `krew-core`: `AgentRuntime` 新增 `language` 字段；`start_completion` 中 identity 构建时注入语言指令
- 无破坏性变更，无新依赖
