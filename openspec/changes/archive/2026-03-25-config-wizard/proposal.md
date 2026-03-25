## Why

krew-cli 目前的上手流程要求用户手动编写 TOML 配置文件（供应商认证、Agent 定义、reply_order 等），即使最简配置也需 ~15 行 TOML 并翻阅文档。这对新用户是显著的上手阻力。参考 OpenClaw 的 `onboard` 向导模式，krew 需要一套交互式配置管理命令，让用户通过「选择」而非「输入」完成配置，实现零手写 TOML 即可开始使用。

## What Changes

- 新增 `krew config init` 子命令：交互式向导，智能检测已有配置状态，分流到 user 级供应商配置或 project 级 Agent 配置
- 新增 `krew config add provider / agent`：向已有配置追加供应商或 Agent
- 新增 `krew config del provider / agent`：从已有配置删除供应商或 Agent
- 新增 `krew config list providers / agents`：列出当前配置的供应商或 Agent
- 新增 `krew config doctor`：诊断配置完整性（API key 是否设置、provider 引用是否有效等）
- `krew-config` 新增配置写入能力（基于 `toml_edit`，格式保留编辑）
- `krew-llm` 新增 `list_models()` API：调用各供应商的 List Models 端点获取可用模型列表，支持降级到硬编码 fallback 列表
- 新增智能预设：根据已配置的供应商动态生成 Agent 组合方案（单 Agent / 三 Agent 两种预设）

## Capabilities

### New Capabilities
- `config-wizard-init`: 交互式配置初始化向导（user 级供应商 + project 级 Agent），包含智能分流、循环添加、预设选择
- `config-wizard-crud`: 配置的增删查操作（add/del/list provider 和 agent）
- `config-wizard-doctor`: 配置诊断，交叉校验供应商、API key、Agent 引用的完整性
- `config-file-writer`: 基于 toml_edit 的格式保留配置文件写入能力
- `llm-list-models`: 各供应商 List Models API 调用 + fallback 硬编码列表

### Modified Capabilities
- `cli-args`: 新增 `config` 子命令组（init / add / del / list / doctor），原有的无子命令行为（直接进入 TUI）保持不变

## Impact

- **涉及 crate**：`krew-cli`（子命令 + 交互流程）、`krew-config`（toml_edit 写入）、`krew-llm`（list_models API）
- **新增依赖**：`dialoguer`（交互式选择/输入/确认）、`toml_edit`（格式保留 TOML 编辑）
- **配置文件**：向 `~/.krew/settings.toml`（user 级）和 `.krew/settings.toml`（project 级）写入内容
- **不涉及**：TUI 主流程、Agent Loop、消息路由等核心运行时逻辑
