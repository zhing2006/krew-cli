## ADDED Requirements

### Requirement: add provider 命令
`krew config add provider` SHALL 向 `~/.krew/settings.toml` 追加一个新的供应商配置。交互流程与 User Init 中添加单个供应商的流程完全一致。

#### Scenario: 添加供应商到已有配置
- **WHEN** `~/.krew/settings.toml` 已包含一个供应商，用户执行 `krew config add provider`
- **THEN** SHALL 完成交互后将新供应商追加到配置文件，不影响已有供应商

#### Scenario: user 配置文件不存在时自动创建
- **WHEN** `~/.krew/settings.toml` 不存在
- **THEN** SHALL 自动创建文件并写入新供应商

#### Scenario: 名称冲突
- **WHEN** 用户输入的供应商名称已存在
- **THEN** SHALL 提示名称已存在，要求重新输入

### Requirement: del provider 命令
`krew config del provider` SHALL 从 `~/.krew/settings.toml` 中删除一个供应商配置。

#### Scenario: 选择并删除供应商
- **WHEN** user 配置中有 3 个供应商
- **THEN** SHALL 显示 `Select` 列出所有供应商供用户选择

#### Scenario: 删除前检查 Agent 引用
- **WHEN** 用户选择删除的供应商被当前项目的 Agent 引用
- **THEN** SHALL 显示警告 "The following agents use this provider: xxx, yyy"，然后要求 `Confirm` 确认删除

#### Scenario: 删除后无引用警告
- **WHEN** 用户选择删除的供应商未被任何 Agent 引用
- **THEN** SHALL 直接要求 `Confirm` 确认删除，不显示引用警告

#### Scenario: 配置中无供应商
- **WHEN** user 配置中没有任何供应商
- **THEN** SHALL 提示 "No providers to delete." 并退出

#### Scenario: 删除后保留其他配置内容
- **WHEN** 删除一个供应商
- **THEN** 文件中其他供应商和非供应商配置 SHALL 保持不变（包括注释和格式）

### Requirement: add agent 命令
`krew config add agent` SHALL 向 `.krew/settings.toml` 追加一个新的 Agent 配置。交互流程与手动创建单个 Agent 的流程一致。

#### Scenario: 添加 Agent 到已有配置
- **WHEN** `.krew/settings.toml` 已包含 Agent，用户执行 `krew config add agent`
- **THEN** SHALL 将新 Agent 追加到 `[[agents]]` 数组和 `reply_order` 末尾

#### Scenario: project 配置文件不存在时自动创建
- **WHEN** `.krew/settings.toml` 不存在
- **THEN** SHALL 自动创建 `.krew/` 目录和配置文件，写入新 Agent 及必要的 `[settings]` 节

#### Scenario: 名称冲突
- **WHEN** 用户输入的 Agent 名称已存在于 project 配置
- **THEN** SHALL 提示名称已存在，要求重新输入

#### Scenario: 供应商列表来源
- **WHEN** 执行 `krew config add agent`
- **THEN** 供应商选择列表 SHALL 读取 merge 后的配置（user `~/.krew/settings.toml` + project `.krew/settings.toml`），与运行时 merge 逻辑一致

#### Scenario: 供应商来自 user 和 project 两层
- **WHEN** user 配置有 `anthropic`，project 配置有 `deepseek`
- **THEN** 供应商选择列表 SHALL 同时包含 `anthropic` 和 `deepseek`

#### Scenario: 无可用供应商
- **WHEN** merge 后的配置中没有任何供应商
- **THEN** SHALL 提示 "No providers configured. Run `krew config add provider` first." 并退出

### Requirement: del agent 命令
`krew config del agent` SHALL 从 `.krew/settings.toml` 中删除一个 Agent 配置。

#### Scenario: 选择并删除 Agent
- **WHEN** project 配置中有多个 Agent
- **THEN** SHALL 显示 `Select` 列出所有 Agent 供用户选择

#### Scenario: 删除后同步 reply_order
- **WHEN** 删除名为 `gpt` 的 Agent
- **THEN** SHALL 同时从 `reply_order` 中移除 `gpt`

#### Scenario: 删除最后一个 Agent
- **WHEN** project 配置中只有一个 Agent，用户选择删除
- **THEN** SHALL 显示警告 "This is the last agent. Deleting it will prevent krew from starting."，要求 `Confirm` 确认

#### Scenario: 配置中无 Agent
- **WHEN** project 配置中没有任何 Agent
- **THEN** SHALL 提示 "No agents to delete." 并退出

### Requirement: list providers 命令
`krew config list providers` SHALL 读取 `~/.krew/settings.toml` 并以表格形式展示所有供应商。

#### Scenario: 列出供应商
- **WHEN** user 配置中有供应商
- **THEN** SHALL 输出表格包含列：名称、类型、Key 方式、Base URL

#### Scenario: 显示环境变量状态
- **WHEN** 供应商使用 `api_key_env` 方式
- **THEN** SHALL 检查该环境变量是否已设置，标记 ✅ 或 ❌

#### Scenario: 无供应商
- **WHEN** user 配置中没有供应商或文件不存在
- **THEN** SHALL 提示 "No providers configured. Run `krew config add provider` to add one."

### Requirement: list agents 命令
`krew config list agents` SHALL 读取 `.krew/settings.toml` 并以表格形式展示所有 Agent。

#### Scenario: 列出 Agent
- **WHEN** project 配置中有 Agent
- **THEN** SHALL 输出表格包含列：名称、显示名、供应商、模型、颜色、Thinking、Web Search

#### Scenario: 显示 reply_order
- **WHEN** 列出 Agent
- **THEN** SHALL 在表格下方显示 `reply_order` 信息（用箭头连接）

#### Scenario: 无 Agent
- **WHEN** project 配置中没有 Agent 或文件不存在
- **THEN** SHALL 提示 "No agents configured. Run `krew config init` or `krew config add agent` to add one."
