## 1. Sub-Agent Discovery & Parse

- [ ] 1.1 新增 `crates/krew-core/src/sub_agent/` 模块：`mod.rs`（pub mod）、`types.rs`（`SubAgentDef` struct）、`discovery.rs`（发现+解析逻辑）
- [ ] 1.2 实现 `discover_sub_agents(cwd: &Path) -> Vec<SubAgentDef>` — 调用 `discovery::discovery_paths(cwd, "agents")`，扫描各目录顶层 `*.md` 文件，解析 YAML frontmatter（name + description 必需，color/maxTurns 可选，其余忽略），body 作为 system_prompt，first-found-wins 去重
- [ ] 1.3 实现 `build_sub_agent_catalog(defs: &[SubAgentDef]) -> String` — 生成 XML 格式的 Sub-Agent catalog 字符串，用于注入 Peer Agent system prompt
- [ ] 1.4 为 discovery 和 parse 编写单元测试（有效文件、缺失字段、Claude Code 兼容字段忽略、优先级去重）

## 2. run_agent Tool

- [ ] 2.1 新增 `crates/krew-tools/src/builtin/run_agent.rs` — 实现 `RunAgentTool` struct 和 `ToolHandler` trait
- [ ] 2.2 实现 `RunAgentTool::spec()` — 返回 tool schema，`agent` 参数 enum 动态填充已发现的 Sub-Agent 名称，`task` 参数为字符串
- [ ] 2.3 实现 `RunAgentTool::execute()` — 查找 SubAgentDef → 构建 AgentRuntime（继承父 Agent 配置，替换 system prompt，排除 run_agent tool） → 构建隔离 messages → start_completion → 消费 event 流 → 通过 output_tx 转发 tool 调用过程 → 返回 final_text
- [ ] 2.4 扩展 `ToolContext`：新增 `event_tx: Option<mpsc::UnboundedSender<AgentEvent>>` 字段用于转发 Sub-Agent 的 `ApprovalRequest` 事件
- [ ] 2.5 修改 `agent_loop.rs` 中的 `create_tool_context()` — 为 `run_agent` tool 设置 `output_tx`（流式输出）和 `event_tx`（审批转发）

## 3. Integration & Registration

- [ ] 3.1 修改 `crates/krew-tools/src/builtin/mod.rs` — 新增 `run_agent` 模块导出，在 `create_full_registry()` 中条件注册 `RunAgentTool`（仅当有 Sub-Agent 定义时）
- [ ] 3.2 修改 `crates/krew-core/src/agent/init.rs` — 在 `init_agents()` 中调用 `discover_sub_agents()`，将结果传给 `create_full_registry()` 和 `build_sub_agent_catalog()`
- [ ] 3.3 修改 `crates/krew-core/src/agent/mod.rs` 的 `build_system_prompt()` — 在 skill catalog 之后注入 Sub-Agent catalog
- [ ] 3.4 修改 `/agents` 命令 — 在 Peer Agent 列表后展示已发现的 Sub-Agent 定义（名称、描述、来源路径）

## 4. Documentation

- [ ] 4.1 更新 `docs/PDD.md` — 新增 Sub-Agent 产品功能章节
- [ ] 4.2 更新 `docs/TDD.md` — 新增 Sub-Agent 技术架构章节（发现机制、run_agent 实现、事件转发）
- [ ] 4.3 更新 `README.md`（中文）— feature 列表新增 Sub-Agent，新增使用示例
- [ ] 4.4 更新 `README_EN.md`（英文）— 对应的英文更新
- [ ] 4.5 更新 `docs/MANUAL.md`（中文）— 新增 Sub-Agent 使用指南章节（定义格式、.claude/agents/ 兼容、使用示例）
- [ ] 4.6 更新 `docs/MANUAL_EN.md`（英文）— 对应的英文更新

## 5. Version Bump

- [ ] 5.1 更新 6 个 Cargo crate 的 `Cargo.toml` version 至 `0.8.0`
- [ ] 5.2 更新 6 个 npm package 的 `package.json` version 至 `0.8.0`（含主包 optionalDependencies）
- [ ] 5.3 更新 CLAUDE.md 中的版本引用（如有）

## 6. Verification

- [ ] 6.1 `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings` 通过
- [ ] 6.2 `cargo test` 全部通过
- [ ] 6.3 `cargo build --release` 成功
- [ ] 6.4 手动测试：在 `.claude/agents/` 放置测试 agent 定义，启动 krew-cli，验证 `/agents` 展示、`run_agent` tool 调用、流式输出展示、审批转发均正常工作
