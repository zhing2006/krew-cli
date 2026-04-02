## 1. SlashCommand 注册

- [ ] 1.1 在 `SlashCommand` enum 中新增 `Dream` 变体，包含 scope（`DreamScope` enum: `Global`/`Agent`/`All`）和 agent name（`String`），在 `from_input()` 中解析 `/dream <scope> @<agent>`
- [ ] 1.2 在 `name()`、`description()`、`all_help()` 中添加 `/dream` 条目

## 2. Dream 模块

- [ ] 2.1 创建 `crates/krew-core/src/dream.rs`，定义 `DreamScope` enum 和 `build_dream_prompt(scope, agent_name)` 函数
- [ ] 2.2 实现 `build_dream_prompt`：根据 scope 动态生成 consolidation prompt（3 阶段：Orient → Consolidate → Prune），包含正确的目录路径
- [ ] 2.3 在 `crates/krew-core/src/lib.rs` 中注册 `pub mod dream`

## 3. TUI 层命令执行

- [ ] 3.1 在 `App` state 中添加 `pending_dream` 字段（存储 scope + agent 列表），在 event loop 中处理
- [ ] 3.2 实现 `execute_dream()` 方法：校验参数（scope/agent 合法性、`@all` 约束、tools 启用检查），设置 pending_dream 状态
- [ ] 3.3 实现 `run_dream()` 异步方法：构建 dream prompt → 注入 user message → 设置 pending_agents 队列 → 调用 `start_next_agent()`

## 4. @all 串行执行

- [ ] 4.1 在 event loop 中处理 `pending_dream` 的 `@all` 情况：从 reply_order 逐个取出 agent，为每个 agent 构建独立的 dream prompt 并注入执行，前一个 agent 完成后再启动下一个

## 5. 单元测试

- [ ] 5.1 测试 `SlashCommand::from_input` 对各种 `/dream` 格式的解析（完整命令、无参数、缺少 agent）
- [ ] 5.2 测试 `DreamScope` 解析和 `@all` 约束校验
- [ ] 5.3 测试 `build_dream_prompt` 对三种 scope 生成的 prompt 内容（目录路径正确、阶段结构正确）
