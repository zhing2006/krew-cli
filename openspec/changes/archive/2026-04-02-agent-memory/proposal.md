## Why

当前 krew-cli 每次会话都是"无记忆"的——Agent 不了解用户的角色、偏好，也不知道之前对话中积累的项目上下文和行为反馈。用户不得不在每次新会话中重复相同的背景信息和行为纠正。

引入持久化 Memory 系统，让每个 Agent 能够跨会话积累对用户、项目和自身行为偏好的认知，显著提升多轮协作体验。

## What Changes

- 新增两层 Memory 存储结构：
  - **Global Memory**（`.krew/memory/`）：存储 `user`、`project`、`reference` 类型记忆，所有 Agent 共享
  - **Per-Agent Memory**（`.krew/memory/agents/{agent_name}/`）：存储 `feedback` 类型记忆，仅该 Agent 可见
- 每次 agent turn 时自动加载 Global MEMORY.md 和当前 Agent 的 MEMORY.md 索引内容，注入 system prompt
- 在 system prompt 中注入 Memory 读写指令（仅 `tools=true` 的 Agent），Agent 通过已有的 `read_file` / `write_file` 工具主动读写记忆文件
- 记忆文件为纯 Markdown 格式，系统不解析文件内容，MEMORY.md 作为索引文件
- 大小限制：MEMORY.md 最多 200 行 / 25KB
- `.krew/memory/**` 路径豁免 DANGEROUS_DIRECTORIES 审批，Agent 读写记忆无需用户确认

## Capabilities

### New Capabilities

- `agent-memory`: Agent Memory 系统的核心能力——记忆的存储结构、读写规则、MEMORY.md 大小限制、approval carve-out
- `agent-memory-prompt`: Memory 指令的 system prompt 注入——记忆指令模板、MEMORY.md 内容加载与截断、tools=false 条件处理、注入到 `build_system_prompt()` 的集成逻辑

### Modified Capabilities

- `agent-loop`: Agent Loop 的 `build_system_prompt()` 需要在每次 agent turn 时加载 Memory 内容并注入 system prompt

## Impact

- **代码影响**：主要修改 `krew-core` crate 的 `agent/mod.rs`（system prompt 构建）和 `agent/approval.rs`（memory 路径 carve-out），新增 memory 模块处理加载与截断逻辑
- **文件系统**：在项目 `.krew/` 目录下新增 `memory/` 子目录结构
- **依赖**：无新外部依赖，复用已有的文件 I/O 能力
- **工具系统**：无变更，复用已有的 `read_file` / `write_file` 工具
- **Breaking Changes**：无
