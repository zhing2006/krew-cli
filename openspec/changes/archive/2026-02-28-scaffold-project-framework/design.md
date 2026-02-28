## Context

项目处于零代码状态，仅有 PDD 和 TDD 设计文档。TDD 已详细定义了 6-crate workspace 架构、所有核心 trait/struct/enum 以及完整的依赖清单。本次工作是将 TDD 中的架构设计落地为可编译的 Rust 代码骨架。

约束：
- 只搭建框架，所有业务方法用 `todo!()` 占位
- 必须通过 `cargo check`、`cargo fmt`、`cargo clippy`
- 严格遵循 TDD §6（项目结构）和 §7（依赖项）的定义

## Goals / Non-Goals

**Goals:**
- 建立完整的 Cargo workspace，包含 6 个 crate 及其正确的依赖关系
- 定义所有核心类型（trait、struct、enum）的签名，确保编译通过
- 生成可用的 `Cargo.lock`，锁定所有依赖版本
- 为后续功能实现提供可直接开发的代码基础

**Non-Goals:**
- 不实现任何业务逻辑（LLM API 调用、工具执行、TUI 渲染等）
- 不编写测试用例（框架阶段无可测试行为）
- 不创建 CI/CD 配置
- 不集成 MCP 协议

## Decisions

### 1. 类型定义集中在各 crate 的 lib.rs/types.rs 中

**选择**: 核心类型直接在 `lib.rs` 中定义并 pub 导出，复杂类型分拆到子模块（如 `krew-llm/src/types.rs`）。

**理由**: 框架阶段代码量小，过度分拆模块会增加样板代码。后续实现时可按需重构。

### 2. trait 方法使用 `todo!()` 或提供默认空实现

**选择**: trait 定义只含方法签名；struct 的 impl 方法使用 `todo!()` 标注为未实现。

**理由**: 让编译器验证类型签名正确性，同时明确标记所有待实现点。

### 3. 依赖版本直接使用 TDD 指定版本

**选择**: workspace dependencies 按 TDD §7.1 声明，不做版本升级。

**理由**: 与 TDD 保持一致，避免引入兼容性问题。实际 `cargo update` 会锁定到该范围内的最新 patch 版本。

### 4. krew-cli 入口使用 clap derive 定义参数

**选择**: `main.rs` 中用 `#[derive(Parser)]` 定义 CLI 参数结构，main 函数解析参数后立即退出。

**理由**: 验证 clap 依赖可用，同时为后续 app 逻辑提供参数结构。

### 5. 按 TDD §6 的模块划分创建源文件

**选择**: 严格按 TDD §6 定义的文件结构创建所有 `.rs` 文件。

**理由**: 保持与设计文档一致，减少后续开发时的结构调整。

## Risks / Trade-offs

- **[Risk] 依赖版本冲突** → 使用 `default-features = false` 最小化 feature 引入；构建时如遇冲突，调整 features 解决
- **[Risk] `todo!()` 导致运行时 panic** → 这是预期行为，框架阶段不运行任何业务逻辑
- **[Trade-off] 类型签名可能需要随实现调整** → 框架阶段优先让编译通过，后续按需修改
