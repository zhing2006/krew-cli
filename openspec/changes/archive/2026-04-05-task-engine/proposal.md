## Why

当前 `run_agent_loop` 是 `pub(crate)` 的内部函数。每次需要"让 AI 自主执行一段工作"的场景（dream 记忆整理、fetch 智能抓取、代码分析等）都必须手动组装 `AgentLoopContext`、管理 channel、收集结果，样板代码重复。将这些样板逻辑封装为 `input → agent loop → output` 的底层 wrapper，是支撑后续所有"内部任务"场景的基础设施。

## What Changes

- 新增 `krew-core::task` 模块，提供 `TaskRequest` / `TaskResult` 类型和 `run_task()` / `run_task_with_events()` 函数
- `run_task()` 封装 channel 创建、消息构建、`AgentLoopContext` 组装、结果收集等样板逻辑
- 权限策略和工具暴露完全由调用方显式传入，底层 wrapper 不做策略假设
- **不改变**任何现有类型/函数的可见性
- **不改造**任何现有功能（dream、sub_agent 等保持原样）

## Capabilities

### New Capabilities
- `task-engine`: 将 agent loop 样板逻辑封装为底层可复用的任务执行 wrapper

### Modified Capabilities
- `agent-loop`: 确认现有 `pub(crate)` 可见性已满足 crate 内新模块的访问需求（无实际代码变更）

## Impact

- **代码**: 新增 `krew-core/src/task/` 模块（~100 行）
- **API**: 新增 `krew_core::task::run_task()` 和 `run_task_with_events()`（`pub` 可见性，供集成测试和未来外部调用方使用）
- **依赖**: 无新外部依赖
- **现有功能**: 零影响
