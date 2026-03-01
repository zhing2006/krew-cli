## Why

Phase 1 完成了 TUI Echo 模式，但所有 Agent 信息是硬编码的。用户无法通过配置文件定义 Agent/Provider，也无法通过 CLI 参数覆盖配置。Phase 2 实现配置系统后，krew 将能从 `.krew/settings.toml` 加载真实的 Agent 定义，并在启动 banner 中展示实际配置的 Agent 列表。

## What Changes

- 在 `krew-config` 中实现 `Config::load()` 函数，从 `.krew/settings.toml` 读取并反序列化为 `Config` 结构体
- 在 `krew-config` 中实现内置默认配置（`defaults.rs`），配置文件不存在时提供 fallback
- 添加配置校验逻辑：验证 `reply_order` 引用的 Agent 存在、Agent 引用的 Provider 存在、必填字段完整
- 在 `krew-cli` 中实现 CLI 参数覆盖：`--agents` 过滤 Agent 列表、`--approval-mode` 覆盖审批策略、`--config` 指定配置路径
- 更新启动 banner，显示实际加载的 Agent 列表（名称 + 颜色）
- 提供清晰的错误提示：配置文件格式错误、字段缺失、引用不合法等
- 在项目根目录创建 `config.example.toml` 示例配置文件

## Capabilities

### New Capabilities
- `config-loading`: 配置文件加载、反序列化、默认值、错误处理
- `config-validation`: 配置校验逻辑（Agent/Provider 引用完整性、必填字段）
- `config-cli-override`: CLI 参数覆盖配置文件设定（--agents, --approval-mode, --config）
- `startup-banner`: 启动 banner 显示实际 Agent 列表和颜色

### Modified Capabilities
- `cli-args`: CLI 参数新增 `--config` 路径解析、`--agents` 过滤逻辑、`--approval-mode` 枚举解析

## Impact

- **krew-config crate**: 新增 `Config::load()`、`Config::default()`、`Config::validate()` 方法，新增 `defaults.rs` 实现
- **krew-cli crate**: `main.rs` 启动流程修改，`app.rs` 新增 `Config` 字段，`render.rs` 更新 banner 渲染
- **krew-core crate**: 无直接修改（已有 `AgentRuntime` 结构但本阶段不初始化 LLM client）
- **项目根目录**: 新增 `config.example.toml` 示例文件
