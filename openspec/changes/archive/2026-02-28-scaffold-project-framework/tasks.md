## 1. Workspace 根配置

- [x] 1.1 创建根 `Cargo.toml`，声明 workspace members 和所有 `[workspace.dependencies]`（按 TDD §7.1）
- [x] 1.2 创建 `.cargo/config.toml` 配置静态链接选项（Windows `static_vcruntime`）

## 2. krew-config crate

- [x] 2.1 创建 `crates/krew-config/Cargo.toml`，依赖 serde、toml、thiserror
- [x] 2.2 创建 `crates/krew-config/src/lib.rs`，定义并导出所有配置类型：Config、Settings、AgentConfig、SamplingConfig、ProviderConfig、McpServerConfig、ApprovalMode、ApiType、McpTrust
- [x] 2.3 创建 `crates/krew-config/src/defaults.rs`，定义内置默认配置占位

## 3. krew-llm crate

- [x] 3.1 创建 `crates/krew-llm/Cargo.toml`，依赖 reqwest、eventsource-stream、futures、serde、serde_json、thiserror、tokio、async-trait
- [x] 3.2 创建 `crates/krew-llm/src/types.rs`，定义 StreamEvent、Usage、OtherAgentRole、ToolDefinition、ChatMessage（LLM 层使用的消息类型）
- [x] 3.3 创建 `crates/krew-llm/src/lib.rs`，定义 LlmClient trait 并导出所有类型和 provider 模块
- [x] 3.4 创建 provider 占位模块：`openai_responses.rs`、`openai_chat.rs`、`openai_compatible.rs`、`anthropic.rs`、`google.rs`

## 4. krew-tools crate

- [x] 4.1 创建 `crates/krew-tools/Cargo.toml`，依赖 tokio、serde、serde_json、thiserror、async-trait
- [x] 4.2 创建 `crates/krew-tools/src/lib.rs`，定义 Tool trait 和 ToolResult struct，导出 builtin 和 mcp 模块
- [x] 4.3 创建 `crates/krew-tools/src/builtin/mod.rs` 及所有内置工具占位文件：read_file.rs、write_file.rs、edit_file.rs、shell.rs、glob.rs、grep.rs
- [x] 4.4 创建 `crates/krew-tools/src/mcp.rs` MCP 客户端占位模块

## 5. krew-storage crate

- [x] 5.1 创建 `crates/krew-storage/Cargo.toml`，依赖 toml、serde、thiserror
- [x] 5.2 创建 `crates/krew-storage/src/lib.rs`，导出 session_file 模块
- [x] 5.3 创建 `crates/krew-storage/src/session_file.rs`，定义会话文件读写函数签名（todo!() 实现）

## 6. krew-core crate

- [x] 6.1 创建 `crates/krew-core/Cargo.toml`，依赖 krew-llm、krew-tools、krew-storage、krew-config、anyhow、serde、serde_json、chrono、uuid、tokio
- [x] 6.2 创建 `crates/krew-core/src/message.rs`，定义 ChatMessage、Role、MessageContent、ContentBlock、ToolCall、ToolCallResult
- [x] 6.3 创建 `crates/krew-core/src/session.rs`，定义 Session struct
- [x] 6.4 创建 `crates/krew-core/src/router.rs`，定义 Addressee enum 和 parse_input 函数签名
- [x] 6.5 创建 `crates/krew-core/src/agent.rs`，定义 AgentRuntime struct
- [x] 6.6 创建 `crates/krew-core/src/command.rs`，定义 SlashCommand enum
- [x] 6.7 创建 `crates/krew-core/src/lib.rs`，导出所有子模块

## 7. krew-cli crate

- [x] 7.1 创建 `crates/krew-cli/Cargo.toml`，依赖 krew-core、krew-config、clap、ratatui、tokio、anyhow、tracing、tracing-subscriber（Windows 下依赖 static_vcruntime）；通过 `[[bin]] name = "krew"` 将可执行文件名设为 `krew`
- [x] 7.2 创建 `crates/krew-cli/src/main.rs`，用 clap derive 定义 CLI 参数结构（Cli struct），main 函数解析参数后退出
- [x] 7.3 创建 `crates/krew-cli/src/app.rs`，定义 App struct 占位
- [x] 7.4 创建 `crates/krew-cli/src/render.rs`，定义渲染模块占位

## 8. 项目配置文件

- [x] 8.1 创建 `config.example.toml` 示例配置文件（按 PDD §4.6.2 的完整配置示例）

## 9. 验证

- [x] 9.1 运行 `cargo fmt --all` 格式化所有代码
- [x] 9.2 运行 `cargo clippy --all-targets --all-features -- -D warnings` 确保无 lint 警告
- [x] 9.3 运行 `cargo check --all-targets` 确保编译通过
