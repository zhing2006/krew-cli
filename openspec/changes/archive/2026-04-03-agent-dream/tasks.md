## 1. SlashCommand 注册

- [x] 1.1 在 `SlashCommand` enum 中新增 `Dream` 变体，包含 scope（`DreamScope` enum: `Global`/`Agent`/`All`）和 agent name（`String`），在 `from_input()` 中解析 `/dream <scope> @<agent>`，拒绝 `@all`
- [x] 1.2 在 `name()`、`description()`、`all_help()` 中添加 `/dream` 条目

## 2. Dream 模块

- [x] 2.1 创建 `crates/krew-core/src/dream.rs`，定义 `DreamScope` enum 和 `build_dream_prompt(scope, agent_name)` 函数
- [x] 2.2 实现 `build_dream_prompt`：根据 scope 动态生成 consolidation prompt（3 阶段：Orient → Consolidate → Prune），包含正确的目录路径。Orient 阶段使用 glob 工具（非 shell ls）
- [x] 2.3 在 `crates/krew-core/src/lib.rs` 中注册 `pub mod dream`

## 3. TUI 层命令执行

- [x] 3.1 实现 `execute_dream()` 方法：校验参数（scope/agent 合法性、拒绝 `@all`、tools 启用检查）
- [x] 3.2 实现 `run_dream()` 方法：构建 dream prompt → 以 whisper message 注入（whisper_targets = [agent_name]，addressee = agent_name）→ 调用 `start_next_agent()` 时传入 `exclude_tools = ["shell", "fetch_url", "activate_skill", "run_agent"]`

## 4. 单元测试

- [x] 4.1 测试 `SlashCommand::from_input` 对各种 `/dream` 格式的解析（完整命令、无参数、缺少 agent、@all 被拒绝）
- [x] 4.2 测试 `DreamScope` 解析校验
- [x] 4.3 测试 `build_dream_prompt` 对三种 scope 生成的 prompt 内容（目录路径正确、阶段结构正确）
