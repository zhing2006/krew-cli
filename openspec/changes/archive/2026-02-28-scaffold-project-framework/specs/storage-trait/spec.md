## ADDED Requirements

### Requirement: 会话存储模块
`krew-storage` SHALL 定义 `session_file` 模块，提供会话 TOML 文件读写的函数。该模块 SHALL 存在函数签名，MAY 使用 `todo!()` 作为实现。

#### Scenario: 存储模块编译通过
- **WHEN** 构建 `krew-storage`
- **THEN** `session_file` 模块 SHALL 编译通过

### Requirement: 存储 crate 导出
`krew-storage` SHALL 从 `lib.rs` 公开导出 `session_file` 模块。

#### Scenario: 存储 crate 可访问
- **WHEN** 导入 `krew_storage::session_file`
- **THEN** 该模块 SHALL 可访问
