## MODIFIED Requirements

### Requirement: main.rs 入口
`krew-cli` SHALL 有一个 `main.rs`，使用 clap 解析 CLI 参数。解析完成后 SHALL 初始化日志系统，然后**加载配置文件**（从 `--config` 指定路径或默认 `.krew/settings.toml`），**应用 CLI 覆盖**（`--agents`、`--approval-mode`），**校验配置**，最后启动 tokio 异步运行时运行 TUI 应用主循环。当 `--config` 显式指定的路径不存在时 SHALL 报错退出；默认路径不存在时 SHALL 静默使用内置默认配置。配置加载 SHALL 在终端初始化之前完成，以便错误信息正常打印到 stderr。终端 SHALL 使用 inline viewport（不切换 alternate screen）。

#### Scenario: 二进制文件可编译运行
- **WHEN** 执行 `cargo run -p krew-cli`
- **THEN** 二进制文件 SHALL 编译通过，在终端当前位置显示 inline TUI 界面

#### Scenario: 使用默认配置启动
- **WHEN** 当前目录下不存在 `.krew/settings.toml`
- **THEN** SHALL 使用内置默认配置启动，不报错

#### Scenario: 指定配置文件路径
- **WHEN** 执行 `cargo run -- --config path/to/custom.toml` 且文件存在
- **THEN** SHALL 从指定路径加载配置文件

#### Scenario: 指定的配置文件不存在
- **WHEN** 执行 `cargo run -- --config nonexistent.toml` 且文件不存在
- **THEN** SHALL 报错退出，提示文件未找到

#### Scenario: 配置文件格式错误
- **WHEN** `.krew/settings.toml` 存在但格式错误
- **THEN** SHALL 显示包含错误位置的提示信息并退出

#### Scenario: --agents 过滤
- **WHEN** 执行 `cargo run -- --agents gpt,opus`
- **THEN** SHALL 仅加载名为 `gpt` 和 `opus` 的 agent

#### Scenario: --approval-mode 覆盖
- **WHEN** 执行 `cargo run -- --approval-mode full-auto`
- **THEN** SHALL 将审批模式设为 `FullAuto`
