## ADDED Requirements

### Requirement: Memory 目录结构
系统 SHALL 使用以下两层目录结构存储记忆文件：

```
.krew/memory/                    ← Global Memory
├── MEMORY.md                    ← Global 索引
├── {topic}.md                   ← Global 记忆文件
└── agents/                      ← Per-Agent Memory
    └── {agent_name}/
        ├── MEMORY.md            ← Agent 索引
        └── {topic}.md           ← Agent 记忆文件
```

- Global 目录（`.krew/memory/`）存储所有 Agent 共享的记忆
- Per-Agent 目录（`.krew/memory/agents/{agent_name}/`）存储仅该 Agent 可见的记忆
- `{agent_name}` SHALL 使用 `AgentConfig.name` 字段值
- 重命名 Agent 不迁移已有记忆，这是可接受的行为

#### Scenario: 首次使用时目录不存在
- **WHEN** 系统加载 Memory 且 `.krew/memory/` 目录不存在
- **THEN** 系统 SHALL 自动创建 `.krew/memory/` 和 `.krew/memory/agents/{agent_name}/` 目录

#### Scenario: 目录已存在
- **WHEN** 系统加载 Memory 且目录已存在
- **THEN** 系统 SHALL 正常读取，不报错

### Requirement: 记忆类型与归属
系统 SHALL 通过 prompt 指令定义四种记忆类型，每种类型有固定的存储归属：

| 类型 | 归属 | 存储路径 |
|------|------|----------|
| `user` | Global | `.krew/memory/` |
| `project` | Global | `.krew/memory/` |
| `reference` | Global | `.krew/memory/` |
| `feedback` | Per-Agent | `.krew/memory/agents/{agent_name}/` |

类型归属由 prompt 指令引导 Agent 行为，系统不解析记忆文件内容或类型。

#### Scenario: user 类型记忆写入 Global
- **WHEN** Agent 按照 prompt 指令创建一条 `user` 类型的记忆
- **THEN** Agent SHALL 将记忆文件写入 `.krew/memory/` 目录，并在 `.krew/memory/MEMORY.md` 中添加索引

#### Scenario: feedback 类型记忆写入 Per-Agent
- **WHEN** Agent `gpt` 按照 prompt 指令创建一条 `feedback` 类型的记忆
- **THEN** Agent SHALL 将记忆文件写入 `.krew/memory/agents/gpt/` 目录，并在 `.krew/memory/agents/gpt/MEMORY.md` 中添加索引

### Requirement: 记忆文件格式
记忆文件 SHALL 为纯 Markdown 格式，由 Agent 通过 `write_file` 工具创建。系统不解析记忆文件内容，文件格式完全由 prompt 指令引导 Agent 行为。

#### Scenario: Agent 创建记忆文件
- **WHEN** Agent 决定保存一条记忆
- **THEN** Agent SHALL 使用 `write_file` 写入一个 `.md` 文件，并更新对应的 MEMORY.md 索引

### Requirement: MEMORY.md 索引格式
MEMORY.md SHALL 作为纯索引文件，每行一条指向记忆文件的链接：

```markdown
- [Title](file.md) — 一行描述
```

- 每条索引 SHALL 不超过 150 个字符
- MEMORY.md SHALL 不直接包含记忆内容，仅包含指向 topic 文件的指针

#### Scenario: 索引指向记忆文件
- **WHEN** Agent 写入一条新记忆
- **THEN** Agent SHALL 同时在对应目录的 MEMORY.md 中添加一行索引

### Requirement: MEMORY.md 大小限制
系统 SHALL 对 MEMORY.md 的加载实施以下限制：

| 限制项 | 值 |
|--------|------|
| MEMORY.md 最大行数 | 200 |
| MEMORY.md 最大字节 | 25,000 |

#### Scenario: MEMORY.md 超出行数限制
- **WHEN** MEMORY.md 内容超过 200 行
- **THEN** 系统 SHALL 截断到 200 行，并附加警告信息提示内容被截断

#### Scenario: MEMORY.md 超出字节限制
- **WHEN** MEMORY.md 内容超过 25,000 字节
- **THEN** 系统 SHALL 截断到最后一个完整行（不超过 25,000 字节），并附加警告信息

### Requirement: Memory 路径 Approval Carve-out
`.krew/memory/**` 路径 SHALL 豁免 DANGEROUS_DIRECTORIES 保护检查。Agent 对 `.krew/memory/` 及其子目录下文件的 `read_file`、`write_file`、`edit_file` 操作 SHALL 自动放行，不弹出审批提示。

该 carve-out SHALL 仅在 bypass immunity 检查阶段（Step 1）生效，在 deny rules 检查（Step 0）之后。用户仍可通过 deny_rules 显式禁止 memory 路径的写入。

#### Scenario: Agent 写入 memory 文件自动放行
- **WHEN** Agent 调用 `write_file` 写入 `.krew/memory/user_role.md`
- **THEN** SHALL 自动放行，不触发审批提示

#### Scenario: Agent 读取 memory 文件自动放行
- **WHEN** Agent 调用 `read_file` 读取 `.krew/memory/agents/gpt/MEMORY.md`
- **THEN** SHALL 自动放行，不触发审批提示

#### Scenario: deny_rules 仍可覆盖 carve-out
- **WHEN** 用户在 settings.toml 中配置了 `deny_rules` 包含 `.krew/memory/**`
- **THEN** deny_rules SHALL 优先于 carve-out，操作被拒绝

#### Scenario: 非 memory 的 .krew 路径仍受保护
- **WHEN** Agent 调用 `write_file` 写入 `.krew/settings.toml`
- **THEN** SHALL 继续触发 DANGEROUS_DIRECTORIES 审批提示

### Requirement: 不应保存的内容
Memory prompt 指令 SHALL 明确列出以下不应保存的内容类型：

- 代码模式、架构、文件路径——可通过阅读代码推导
- Git 历史——使用 `git log` 查询
- 调试解决方案——修复已在代码中
- 项目指令文件中已有的内容
- 临时任务细节、当前会话上下文

#### Scenario: Agent 收到保存代码模式的请求
- **WHEN** 用户要求 Agent 保存代码模式或架构信息
- **THEN** Agent SHALL 询问用户哪部分是"令人意外的或非显而易见的"，仅保存该部分
