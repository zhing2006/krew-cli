## ADDED Requirements

### Requirement: 6-crate Cargo workspace
项目 SHALL 是一个 Cargo workspace，包含 6 个成员 crate：`krew-cli`、`krew-core`、`krew-llm`、`krew-tools`、`krew-storage`、`krew-config`，全部位于 `crates/` 目录下。

#### Scenario: workspace 成员正确解析
- **WHEN** 在 workspace 根目录执行 `cargo check --all-targets`
- **THEN** 所有 6 个 crate SHALL 编译成功，无错误

### Requirement: workspace 依赖集中管理
所有共享依赖 SHALL 在根 `Cargo.toml` 的 `[workspace.dependencies]` 中声明，使用 `default-features = false`，成员 crate 通过 `workspace = true` 引用。

#### Scenario: 无重复依赖版本
- **WHEN** 检查 `Cargo.lock`
- **THEN** 共享 crate（tokio、serde、serde_json、anyhow、thiserror、chrono、uuid、toml、reqwest、futures）SHALL 各自最多出现一个版本

### Requirement: crate 依赖关系符合 TDD
crate 间依赖关系 SHALL 遵循：`krew-cli` 依赖 `krew-core` 和 `krew-config`；`krew-core` 依赖 `krew-llm`、`krew-tools`、`krew-storage` 和 `krew-config`。

#### Scenario: 依赖链编译通过
- **WHEN** 构建 `krew-cli`
- **THEN** 通过 `krew-core` 的所有传递依赖 SHALL 正确解析，无循环引用

### Requirement: Rust edition 2024
所有 crate SHALL 在 `Cargo.toml` 中使用 `edition = "2024"`。

#### Scenario: edition 字段设置正确
- **WHEN** 检查任意 crate 的 `Cargo.toml`
- **THEN** `edition` 字段 SHALL 为 `"2024"`
