## ADDED Requirements

### Requirement: 版本号升级至 v0.8.0
所有版本标识 SHALL 从当前版本升级至 `0.8.0`：

- 6 个 Cargo crate 的 `Cargo.toml`（`version = "0.8.0"`）
- 6 个 npm package 的 `package.json`（`"version": "0.8.0"` + 主包的 `optionalDependencies` 版本）

#### Scenario: Cargo crate 版本一致
- **WHEN** 构建完成后检查所有 6 个 crate 的 `Cargo.toml`
- **THEN** 每个文件的 `version` 字段 SHALL 为 `"0.8.0"`

#### Scenario: npm package 版本一致
- **WHEN** 检查所有 6 个 `package.json`
- **THEN** 每个文件的 `version` 字段 SHALL 为 `"0.8.0"`，主包 `npm/krew/package.json` 的 5 个 `optionalDependencies` 版本 SHALL 为 `"0.8.0"`

### Requirement: 文档更新
以下文档 SHALL 新增 Sub-Agent 相关章节：

- `docs/PDD.md`：产品设计文档，新增 Sub-Agent 功能说明
- `docs/TDD.md`：技术设计文档，新增 Sub-Agent 架构说明
- `README_CN.md`（中文）和 `README.md`（英文）：新增 Sub-Agent feature 描述
- `docs/MANUAL_CN.md`（中文）和 `docs/MANUAL.md`（英文）：新增 Sub-Agent 使用指南

#### Scenario: PDD 包含 Sub-Agent 功能
- **WHEN** 查看 `docs/PDD.md`
- **THEN** SHALL 包含 Sub-Agent 的产品定义——定义方式、调用机制、上下文隔离的价值

#### Scenario: TDD 包含 Sub-Agent 架构
- **WHEN** 查看 `docs/TDD.md`
- **THEN** SHALL 包含 Sub-Agent 的技术设计——发现机制、`run_agent` tool 实现、事件转发

#### Scenario: README 包含 Sub-Agent feature
- **WHEN** 查看 `README_CN.md` 和 `README.md`
- **THEN** SHALL 在 feature 列表中提及 Sub-Agent 功能

#### Scenario: MANUAL 包含使用指南
- **WHEN** 查看 `docs/MANUAL_CN.md` 和 `docs/MANUAL.md`
- **THEN** SHALL 包含 Sub-Agent 的定义文件格式说明、使用示例、以及 `.claude/agents/` 兼容说明
