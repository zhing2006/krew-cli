## MODIFIED Requirements

### Requirement: main.rs 入口
`krew-cli` SHALL 有一个 `main.rs`，使用 clap 解析 CLI 参数。解析完成后 SHALL 初始化日志系统，然后启动 tokio 异步运行时运行 TUI 应用主循环。终端 SHALL 使用 inline viewport（不切换 alternate screen）。

#### Scenario: 二进制文件可编译运行
- **WHEN** 执行 `cargo run -p krew-cli`
- **THEN** 二进制文件 SHALL 编译通过，在终端当前位置显示 inline TUI 界面

### Requirement: app、render 和 custom_terminal 模块存在
`krew-cli` SHALL 包含 `app.rs`、`render.rs` 和 `custom_terminal.rs` 源文件。`app.rs` SHALL 包含 App 状态机和主事件循环逻辑。`render.rs` SHALL 包含 TUI 渲染逻辑。`custom_terminal.rs` SHALL 提供支持动态 viewport 高度的自定义 Terminal 实现。

#### Scenario: CLI 模块编译通过
- **WHEN** 构建 `krew-cli`
- **THEN** `app`、`render` 和 `custom_terminal` 三个模块 SHALL 编译通过并包含功能实现
