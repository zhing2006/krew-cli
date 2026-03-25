## MODIFIED Requirements

### Requirement: main.rs 入口
`krew-cli` SHALL 有一个 `main.rs`，使用 clap 解析 CLI 参数。当无子命令时，SHALL 保持现有行为：初始化日志系统，加载配置文件，应用 CLI 覆盖，校验配置，启动 tokio 异步运行时运行 TUI 应用主循环。当子命令为 `config` 时，SHALL 分发到对应的配置管理子命令处理函数。

`krew` 的 clap 定义 SHALL 支持以下子命令结构：
- `krew config init [--user | --project]` — 交互式初始化
- `krew config add <provider | agent>` — 添加供应商或 Agent
- `krew config del <provider | agent>` — 删除供应商或 Agent
- `krew config list <providers | agents>` — 列出供应商或 Agent
- `krew config doctor` — 诊断配置

子命令 SHALL 不初始化 TUI terminal、不加载完整 Config（仅按需读取 user/project 配置文件）。所有 `config` 子命令 SHALL 统一创建一个轻量的 `tokio::runtime::Builder::new_current_thread()` runtime 来执行处理函数，保持实现一致性。

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

#### Scenario: config 子命令不初始化 TUI
- **WHEN** 执行任意 `krew config *` 子命令
- **THEN** SHALL 不调用 `setup_terminal()`，不进入 raw mode，直接在普通终端模式下运行

#### Scenario: init --user 和 --project 互斥
- **WHEN** 执行 `krew config init --user --project`
- **THEN** clap SHALL 在参数解析阶段报错，提示两个标志互斥
