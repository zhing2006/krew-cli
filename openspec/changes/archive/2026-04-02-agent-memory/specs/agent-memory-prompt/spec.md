## ADDED Requirements

### Requirement: Memory Prompt 模板
系统 SHALL 维护一个 Memory 指令模板，用于注入到 Agent 的 system prompt 中。模板 SHALL 包含以下内容：

1. Memory 系统简介——说明两层存储（global / personal）及各自路径
2. 四种记忆类型定义——`user`、`feedback`、`project`、`reference`，每种包含 scope（global/personal）、描述、何时保存
3. 不应保存的内容列表
4. 保存记忆的两步流程（写 topic 文件 + 更新 MEMORY.md 索引）
5. 何时访问记忆的指导
6. 大小限制说明（MEMORY.md 最多 200 行）

模板中的 `{{agent_name}}` 占位符 SHALL 在注入时替换为当前 Agent 的 name。

#### Scenario: 模板变量替换
- **WHEN** 为 Agent `gpt` 构建 Memory prompt
- **THEN** 模板中所有 `{{agent_name}}` SHALL 替换为 `gpt`，路径显示为 `.krew/memory/agents/gpt/`

#### Scenario: 模板包含完整指导
- **WHEN** Memory prompt 注入到 system prompt
- **THEN** SHALL 包含记忆类型定义、归属规则、保存流程、访问指导和大小限制

### Requirement: tools=false Agent 的 Memory 注入
当 Agent 的 `config.tools = false` 时，该 Agent 没有 `read_file` / `write_file` 工具可用。系统 SHALL 对这类 Agent 仅注入只读部分的 Memory prompt——即 MEMORY.md 索引内容，不注入写入指令。

#### Scenario: tools=true Agent 收到完整 Memory prompt
- **WHEN** Agent `gpt` 的 `config.tools = true`
- **THEN** system prompt SHALL 包含完整的 Memory 指令（读写规则 + 索引内容）

#### Scenario: tools=false Agent 仅收到索引内容
- **WHEN** Agent `reader` 的 `config.tools = false`
- **THEN** system prompt SHALL 仅包含 Memory 索引内容（Global MEMORY.md + Per-Agent MEMORY.md），不包含写入指令

### Requirement: Memory 内容加载
`load_memory_prompt()` 函数 SHALL 加载并拼接以下内容：

1. Memory 指令模板（变量替换后）——仅当 `tools=true` 时包含
2. Global MEMORY.md 内容（`.krew/memory/MEMORY.md`），添加 `## Global Memory` 标题
3. Per-Agent MEMORY.md 内容（`.krew/memory/agents/{agent_name}/MEMORY.md`），添加 `## Your Memory` 标题

#### Scenario: 两份 MEMORY.md 都存在
- **WHEN** Global MEMORY.md 和 Per-Agent MEMORY.md 都存在且非空
- **THEN** 输出 SHALL 依次包含：指令模板（如适用）、`## Global Memory` + Global 内容、`## Your Memory` + Agent 内容

#### Scenario: MEMORY.md 不存在
- **WHEN** Global MEMORY.md 或 Per-Agent MEMORY.md 不存在
- **THEN** SHALL 跳过对应段落，不报错

#### Scenario: MEMORY.md 为空
- **WHEN** MEMORY.md 存在但内容为空
- **THEN** SHALL 跳过对应段落

### Requirement: MEMORY.md 截断
`read_and_truncate()` 函数 SHALL 对 MEMORY.md 内容实施截断：

- 先按行数截断（最多 200 行）
- 再按字节截断（最多 25,000 字节），截断位置 SHALL 在最后一个完整行的换行符处
- 截断后 SHALL 附加警告信息，说明被截断的原因（超行 / 超字节 / 两者）

#### Scenario: 内容在限制内
- **WHEN** MEMORY.md 内容少于 200 行且小于 25,000 字节
- **THEN** SHALL 返回完整内容，无警告

#### Scenario: 超出行数限制
- **WHEN** MEMORY.md 有 250 行
- **THEN** SHALL 返回前 200 行 + 警告："MEMORY.md is 250 lines (limit: 200). Only part of it was loaded."

#### Scenario: 超出字节限制
- **WHEN** MEMORY.md 有 180 行但总计 30,000 字节
- **THEN** SHALL 截断到不超过 25,000 字节的最后完整行 + 警告

### Requirement: System Prompt 注入位置
Memory prompt SHALL 注入到 `build_system_prompt()` 的以下位置：

```
Project Instructions → Skill Catalog → Sub-Agent Catalog → 【Memory Prompt】 → Agent Prompt
```

#### Scenario: 有 Memory 内容时注入
- **WHEN** `.krew/memory/` 目录存在
- **THEN** system prompt SHALL 在 Sub-Agent Catalog 之后、Agent Prompt 之前包含 Memory 段

#### Scenario: 无 Memory 目录时跳过
- **WHEN** `.krew/memory/` 目录不存在且无法创建（例如只读文件系统）
- **THEN** system prompt SHALL 不包含 Memory 段，不影响正常功能

### Requirement: Memory 目录自动创建
`load_memory_prompt()` SHALL 在加载记忆前确保目录存在：

- 创建 `.krew/memory/` 目录（如不存在）
- 为当前 Agent 创建 `.krew/memory/agents/{agent_name}/` 目录（如不存在）
- 使用 `create_dir_all` 语义（递归创建，已存在不报错）

#### Scenario: 首次调用时创建目录
- **WHEN** 首次为 Agent `opus` 加载 Memory prompt 且 `.krew/memory/agents/opus/` 不存在
- **THEN** SHALL 创建 `.krew/memory/agents/opus/` 目录

#### Scenario: 目录创建失败
- **WHEN** 目录创建因权限等原因失败
- **THEN** SHALL 跳过 Memory 功能，不中断会话启动
