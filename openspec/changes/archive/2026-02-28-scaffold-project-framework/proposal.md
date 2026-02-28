## Why

项目目前仅有 PDD 和 TDD 设计文档，尚未开始任何代码实现。需要按照 TDD 定义的 6-crate workspace 架构搭建完整的项目骨架，为后续业务功能实现提供可编译、可测试的代码基础设施。

## What Changes

- 创建 Cargo workspace 根配置，声明所有 workspace dependencies
- 创建 6 个 crate 的目录结构和 Cargo.toml（krew-cli、krew-core、krew-llm、krew-tools、krew-storage、krew-config）
- 在每个 crate 中定义核心 trait、struct、enum 的骨架（签名 + `todo!()`/空实现），不实现业务逻辑
- 创建 `main.rs` 入口，使用 clap 定义 CLI 参数结构（不实现交互逻辑）
- 添加示例配置文件 `config.example.toml`
- 确保 `cargo check`、`cargo fmt`、`cargo clippy` 全部通过

## Capabilities

### New Capabilities
- `workspace-setup`: Cargo workspace 根配置与 6 个 crate 的项目结构
- `config-types`: 配置数据结构定义（Config、Settings、AgentConfig、ProviderConfig 等）
- `message-types`: 消息与会话数据模型定义（ChatMessage、Session、Role 等）
- `llm-trait`: LLM Client trait 及 StreamEvent、Usage 类型定义
- `tool-trait`: Tool trait 及 ToolResult 类型定义
- `storage-trait`: 会话持久化接口定义
- `cli-args`: CLI 参数结构定义（clap derive）

### Modified Capabilities

（无 — 这是全新项目初始化）

## Impact

- 建立完整的 crate 依赖关系图（krew-cli → krew-core → krew-llm/krew-tools/krew-storage/krew-config）
- 引入所有 TDD 7.1 中定义的 workspace dependencies
- 后续所有功能开发将基于此框架进行
