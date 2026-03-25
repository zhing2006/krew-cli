## Why

krew 运行时，agent 拥有 read/write/edit/shell 等工具可以修改配置文件，但它们不知道 krew 的配置结构，也不知道自己可以帮用户管理配置。用户目前必须退出 TUI 去跑 `krew config` 子命令或手动编辑 TOML 文件。通过在系统提示词中注入简短提示，并提供 `krew config help` 命令输出完整配置手册，agent 就能在会话中自主协助用户完成配置变更。

## What Changes

- 新增 `krew config help` 子命令，打印一份完整的、硬编码的配置手册，涵盖文件位置、TOML 结构、字段参考和 CLI 命令参考。
- 增强 agent identity prompt，加入一句 krew 简介以及 "可以执行 `krew config help` 获取配置手册" 的提示。

## Capabilities

### New Capabilities
- `config-help-command`: `krew config help` 子命令，以纯文本输出完整的 krew 配置手册。

### Modified Capabilities
- `cli-args`: 在 `config` 子命令中新增 `Help` 变体。
- `agent-loop`: 在 identity prompt 中加入 krew 简介和配置帮助提示。

## Impact

- `crates/krew-cli/src/main.rs` — 在 `ConfigAction` 枚举中新增 `Help` 变体
- `crates/krew-cli/src/config_cmd/mod.rs` — 分发 `Help` 到新处理函数
- `crates/krew-cli/src/config_cmd/help.rs` — 新模块，包含硬编码的手册文本
- `crates/krew-core/src/agent/mod.rs` — 修改 `build_identity_prompt()` 加入 krew 简介和配置帮助提示
