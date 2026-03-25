## ADDED Requirements

### Requirement: main.rs 入口
`krew-cli` SHALL 有一个 `main.rs`，使用 clap 解析 CLI 参数。当无子命令时，SHALL 保持现有行为：初始化日志系统，加载配置文件，应用 CLI 覆盖，校验配置，启动 tokio 异步运行时运行 TUI 应用主循环。当子命令为 `config` 时，SHALL 分发到对应的配置管理子命令处理函数。

`krew` 的 clap 定义 SHALL 支持以下子命令结构：
- `krew config init [--user | --project]` — 交互式初始化
- `krew config add <provider | agent>` — 添加供应商或 Agent
- `krew config del <provider | agent>` — 删除供应商或 Agent
- `krew config list <providers | agents>` — 列出供应商或 Agent
- `krew config doctor` — 诊断配置
- `krew config help` — 打印配置手册

子命令 SHALL 不初始化 TUI terminal、不加载完整 Config（仅按需读取 user/project 配置文件）。所有 `config` 子命令 SHALL 统一创建一个轻量的 `tokio::runtime::Builder::new_current_thread()` runtime 来执行处理函数，保持实现一致性。

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

#### Scenario: 无子命令保持 TUI 行为
- **WHEN** 执行 `krew`（无子命令）
- **THEN** SHALL 保持现有行为，进入 TUI 界面

#### Scenario: 无子命令带参数保持现有行为
- **WHEN** 执行 `krew --agents opus,gpt` 或 `krew -p "hello"` 或 `krew --resume`
- **THEN** SHALL 保持现有行为，不受子命令新增影响

#### Scenario: config 子命令分发
- **WHEN** 执行 `krew config init`
- **THEN** SHALL 调用 init 子命令处理函数

#### Scenario: config add 子命令
- **WHEN** 执行 `krew config add provider`
- **THEN** SHALL 调用 add provider 处理函数

#### Scenario: config add 子命令 (agent)
- **WHEN** 执行 `krew config add agent`
- **THEN** SHALL 调用 add agent 处理函数

#### Scenario: config del 子命令
- **WHEN** 执行 `krew config del provider`
- **THEN** SHALL 调用 del provider 处理函数

#### Scenario: config del 子命令 (agent)
- **WHEN** 执行 `krew config del agent`
- **THEN** SHALL 调用 del agent 处理函数

#### Scenario: config list 子命令
- **WHEN** 执行 `krew config list providers`
- **THEN** SHALL 调用 list providers 处理函数

#### Scenario: config list 子命令 (agents)
- **WHEN** 执行 `krew config list agents`
- **THEN** SHALL 调用 list agents 处理函数

#### Scenario: config doctor 子命令
- **WHEN** 执行 `krew config doctor`
- **THEN** SHALL 调用 doctor 处理函数

#### Scenario: config help 子命令
- **WHEN** 执行 `krew config help`
- **THEN** SHALL 调用 help 处理函数，打印配置手册

#### Scenario: config 子命令不初始化 TUI
- **WHEN** 执行任意 `krew config *` 子命令
- **THEN** SHALL 不调用 `setup_terminal()`，不进入 raw mode，直接在普通终端模式下运行

#### Scenario: init --user 和 --project 互斥
- **WHEN** 执行 `krew config init --user --project`
- **THEN** clap SHALL 在参数解析阶段报错，提示两个标志互斥

### Requirement: --resume CLI argument
The `--resume` CLI argument SHALL resume a session on startup. When provided without an ID (`--resume`), it SHALL resume the most recent session. When provided with an ID (`--resume <id>`), it SHALL resume the specified session.

#### Scenario: Resume most recent session
- **WHEN** the user starts krew with `--resume` (no ID)
- **THEN** the system SHALL load the most recently updated session and display a confirmation with the session ID

#### Scenario: Resume specific session by ID
- **WHEN** the user starts krew with `--resume abc123`
- **THEN** the system SHALL load the session with id "abc123" (matching by prefix)

#### Scenario: Resume with non-existent session ID
- **WHEN** the user starts krew with `--resume nonexistent`
- **THEN** the system SHALL display an error message and start a new session instead

#### Scenario: Resume with no saved sessions
- **WHEN** the user starts krew with `--resume` and no sessions exist
- **THEN** the system SHALL display an info message and start a new session

### Requirement: app、render 和 custom_terminal 模块存在
`krew-cli` SHALL 包含 `app.rs`、`render.rs` 和 `custom_terminal.rs` 源文件。`app.rs` SHALL 包含 App 状态机和主事件循环逻辑。`render.rs` SHALL 包含 TUI 渲染逻辑。`custom_terminal.rs` SHALL 提供支持动态 viewport 高度的自定义 Terminal 实现。

#### Scenario: CLI 模块编译通过
- **WHEN** 构建 `krew-cli`
- **THEN** `app`、`render` 和 `custom_terminal` 三个模块 SHALL 编译通过并包含功能实现
