## ADDED Requirements

### Requirement: CLI 参数结构体
`krew-cli` SHALL 使用 clap derive 宏定义 `Cli` 结构体，包含以下选项：`-c, --config <PATH>`、`-a, --agents <NAMES>`、`--approval-mode <MODE>`、`--resume [ID]`、`-v, --verbose`、`-h, --help`、`-V, --version`。

#### Scenario: CLI 参数解析成功
- **WHEN** 执行 `krew --help`
- **THEN** 所有定义的选项 SHALL 出现在帮助输出中

### Requirement: 可执行文件名为 krew
`krew-cli` crate 的 `Cargo.toml` SHALL 通过 `[[bin]]` 配置将最终可执行文件名设为 `krew`（而非默认的 `krew-cli`）。

#### Scenario: 可执行文件名正确
- **WHEN** 执行 `cargo build -p krew-cli`
- **THEN** 生成的可执行文件 SHALL 命名为 `krew`（Windows 上为 `krew.exe`）

### Requirement: main.rs 入口
`krew-cli` SHALL 有一个 `main.rs`，使用 clap 解析 CLI 参数。在框架阶段，解析完成后 SHALL 正常退出（不执行业务逻辑）。

#### Scenario: 二进制文件可编译运行
- **WHEN** 执行 `cargo run -p krew-cli -- --help`
- **THEN** 二进制文件 SHALL 编译通过并显示帮助文本，无错误

### Requirement: app 和 render 模块存在
`krew-cli` SHALL 包含 TDD §6 中定义的 `app.rs` 和 `render.rs` 源文件，MAY 仅包含占位代码。

#### Scenario: CLI 模块编译通过
- **WHEN** 构建 `krew-cli`
- **THEN** `app` 和 `render` 两个模块 SHALL 编译通过
