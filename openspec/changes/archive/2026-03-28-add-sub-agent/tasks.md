## 1. Settings & Feature Gate

- [x] 1.1 在 `krew-config` 的 `Settings` / `RawSettings` 中新增 `sub_agent_enabled: bool` 字段（默认 `false`），TOML 配置项名为 `sub_agent_enabled`
- [x] 1.2 在配置帮助文本 / 示例配置中补充 `sub_agent_enabled` 字段的说明（`config.example.toml`、`/help` 输出等）
- [x] 1.3 确保 TUI 和 prompt 模式在该开关为 `false` 时完全跳过 Sub-Agent 发现、catalog 注入、tool 注册——零开销

## 2. Sub-Agent Discovery & Parse

- [x] 2.1 新增 `crates/krew-core/src/sub_agent/` 模块：`mod.rs`（pub mod）、`types.rs`（`SubAgentDef` struct）、`discovery.rs`（发现+解析逻辑）
- [x] 2.2 实现 `discover_sub_agents(cwd: &Path) -> Vec<SubAgentDef>` — 调用 `discovery::discovery_paths(cwd, "agents")`，扫描各目录顶层 `*.md` 文件，解析 YAML frontmatter（name + description 必需，color/maxTurns 可选，其余忽略），body 作为 system_prompt，first-found-wins 去重
- [x] 2.3 实现 `build_sub_agent_catalog(defs: &[SubAgentDef]) -> String` — 生成 XML 格式的 Sub-Agent catalog 字符串，用于注入 Peer Agent system prompt
- [x] 2.4 为 discovery 和 parse 编写单元测试（有效文件、缺失字段、Claude Code 兼容字段忽略、优先级去重）

## 3. run_agent Tool（在 krew-core 中实现，注册到 krew-tools ToolRegistry）

- [x] 3.1 新增 `crates/krew-core/src/sub_agent/run_agent_tool.rs` — 实现 `RunAgentTool` struct，impl `krew_tools::ToolHandler` trait
- [x] 3.2 `RunAgentTool` struct 持有以下字段：`defs: HashMap<String, SubAgentDef>`（Sub-Agent 定义）、`is_running: Arc<AtomicBool>`（depth guard 防嵌套标志）、以及父 Agent 的运行时资源（`client: Arc<dyn LlmClient>`、`approval_mode`、`approval_cache`、`sampling` 等）。注意：`ToolRegistry` 和 parent event sender 不在构造时持有，而是通过 `ToolContext.tool_registry` 和 `ToolContext.parent_event_tx` 在每次 execute 时获取（避免 `Arc::get_mut` 注册失败）
- [x] 3.3 实现 `RunAgentTool::spec()` — 返回 tool schema，`agent` 参数 enum 动态填充已发现的 Sub-Agent 名称，`task` 参数为字符串
- [x] 3.4 实现 `RunAgentTool::execute()` — 入口 CAS `is_running`（嵌套则报错）→ 从 `ctx.tool_registry` downcast 取得 `Arc<ToolRegistry>` → 从 `ctx.parent_event_tx` downcast 取得父 Agent sender → 查找 SubAgentDef → 构建隔离 messages `[system, user(task)]` → 构建 tool_defs 时过滤掉 run_agent → 调用 `run_agent_loop` → 循环消费 sub_rx：ToolCallStart/Output/Done 通过 `ctx.output_tx` 转发；ApprovalRequest 通过 parent sender 转发；TextDelta 累积 → Done 时返回 final_text → 退出时重置 `is_running`
- [x] 3.5 修改 `krew-tools::ToolContext` — 新增 `parent_event_tx: Option<Box<dyn Any + Send + Sync>>` 和 `tool_registry: Option<Box<dyn Any + Send + Sync>>` 字段（默认 None）
- [x] 3.6 修改 `krew-core` 的 `create_tool_context()` — 为 `run_agent` tool 设置 `output_tx`（流式输出）、`parent_event_tx`（装箱的父 Agent event sender clone）和 `tool_registry`（装箱的 `Arc<ToolRegistry>` clone）
- [x] 3.7 修改 `AgentRuntime::start_completion()` — 新增 `exclude_tools: Option<&[&str]>` 参数，构建 `tool_defs` 时跳过指定的 tool 名称

## 4. Integration & Registration

- [x] 4.1 修改 `crates/krew-core/src/agent/init.rs` — 在 `init_agents()` 中，当 `sub_agent_enabled` 为 true 时调用 `discover_sub_agents()`，将 `SubAgentDef` 列表存入 `InitAgentsResult`
- [x] 4.2 实现 `register_sub_agents()` 辅助函数 — 接受 `&mut HashMap<String, AgentRuntime>` 和 `Vec<SubAgentDef>`，在每个启用 tools 的 Agent 的 `ToolRegistry` 中注册 `RunAgentTool`（通过 `Arc::get_mut` 获取可变引用，与 MCP 注册方式一致）
- [x] 4.3 在 TUI 模式中，MCP 初始化块之后（同级，无论是否有 MCP 配置），当 `sub_agent_enabled` 且有 Sub-Agent 定义时调用 `register_sub_agents()`（`app/state.rs`）
- [x] 4.4 在 prompt 模式中，MCP 初始化块之后（同级，无论是否有 MCP 配置），当 `sub_agent_enabled` 且有 Sub-Agent 定义时调用 `register_sub_agents()`（`prompt_mode/mod.rs`）
- [x] 4.5 修改 `crates/krew-core/src/agent/mod.rs` 的 `build_system_prompt()` — 在 skill catalog 之后注入 Sub-Agent catalog
- [x] 4.6 修改 `/agents` 命令 — 在 Peer Agent 列表后展示已发现的 Sub-Agent 定义（名称、描述、来源路径）

## 5. Documentation

- [x] 5.1 更新 `docs/PDD.md` — 新增 Sub-Agent 产品功能章节
- [x] 5.2 更新 `docs/TDD.md` — 新增 Sub-Agent 技术架构章节（发现机制、run_agent 实现、事件转发）
- [x] 5.3 更新 `README_CN.md`（中文）— feature 列表新增 Sub-Agent，新增使用示例
- [x] 5.4 更新 `README.md`（英文）— 对应的英文更新
- [x] 5.5 更新 `docs/MANUAL_CN.md`（中文）— 新增 Sub-Agent 使用指南章节（定义格式、.claude/agents/ 兼容、使用示例、`sub_agent_enabled` 开关说明）
- [x] 5.6 更新 `docs/MANUAL.md`（英文）— 对应的英文更新

## 6. Version Bump

- [x] 6.1 更新 6 个 Cargo crate 的 `Cargo.toml` version 至 `0.8.0`
- [x] 6.2 更新 6 个 npm package 的 `package.json` version 至 `0.8.0`（含主包 optionalDependencies）
- [x] ~~6.3 创建 `v0.8.0` git tag~~ — 合并后再打 tag，跳过

## 7. Verification

- [x] 7.1 `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings` 通过
- [x] 7.2 `cargo test` 全部通过
- [x] 7.3 `cargo build --release` 成功
- [x] 7.4 手动测试（`sub_agent_enabled = false`）：验证无任何 Sub-Agent 行为，`/agents` 不展示 Sub-Agent，无 `run_agent` tool
- [x] 7.5 手动测试（`sub_agent_enabled = true`）：在 `.claude/agents/` 放置测试 agent 定义，验证 `/agents` 展示、`run_agent` tool 调用、流式 tool events 展示、审批转发均正常工作
