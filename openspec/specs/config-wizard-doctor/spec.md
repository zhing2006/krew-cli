## ADDED Requirements

### Requirement: doctor 命令基于 merge 后的最终配置诊断
`krew config doctor` SHALL 使用与运行时相同的配置 merge 逻辑（UserConfig + RawConfig → merge → resolve）来获取最终配置，然后进行诊断。这确保 doctor 的诊断结果与实际运行时行为一致。

#### Scenario: 完整诊断流程
- **WHEN** 执行 `krew config doctor`
- **THEN** SHALL 按以下顺序输出诊断结果：配置文件状态 → 供应商状态 → Agent 状态 → MCP 服务器状态（如有）→ 总结

#### Scenario: 配置文件状态
- **WHEN** 两个配置文件都存在
- **THEN** SHALL 显示 `✅ User config: ~/.krew/settings.toml` 和 `✅ Project config: .krew/settings.toml`

#### Scenario: user 配置文件缺失
- **WHEN** `~/.krew/settings.toml` 不存在
- **THEN** SHALL 显示 `❌ User config: ~/.krew/settings.toml (not found)`

#### Scenario: project 配置文件缺失
- **WHEN** `.krew/settings.toml` 不存在
- **THEN** SHALL 显示 `❌ Project config: .krew/settings.toml (not found)`

#### Scenario: project 级供应商参与诊断
- **WHEN** user 配置有供应商 `anthropic`，project 配置有供应商 `deepseek`
- **THEN** 供应商诊断 SHALL 同时检查 `anthropic` 和 `deepseek`（合并后的配置）

#### Scenario: project 级供应商覆盖 user 级
- **WHEN** user 和 project 都定义了 `openai` 供应商，user 用 api_key_env，project 用 api_key
- **THEN** 诊断 SHALL 按 project 级的配置判断（显示 "API key configured (config file)"）

#### Scenario: user 配置文件 TOML 损坏
- **WHEN** `~/.krew/settings.toml` 存在但 TOML 格式错误（无法解析）
- **THEN** SHALL 显示 `❌ User config: ~/.krew/settings.toml (parse error: <error detail>)`
- **AND** SHALL 跳过供应商和 Agent 诊断中依赖 user 配置的部分，但仍继续诊断 project 配置

#### Scenario: project 配置文件 TOML 损坏
- **WHEN** `.krew/settings.toml` 存在但 TOML 格式错误
- **THEN** SHALL 显示 `❌ Project config: .krew/settings.toml (parse error: <error detail>)`
- **AND** SHALL 跳过 Agent 诊断，但仍继续诊断 user 配置中的供应商

#### Scenario: doctor 不使用 UserConfig::load() 的静默回退
- **WHEN** doctor 加载配置文件
- **THEN** SHALL 不使用 `UserConfig::load()` 的静默 fallback 行为（该函数在解析失败时打印 warning 并返回默认值）。doctor SHALL 自行读取并解析配置文件，在解析失败时明确报告错误而非静默忽略

### Requirement: 供应商诊断
doctor SHALL 检查 merge 后每个供应商的 API key 可用性。

#### Scenario: 环境变量方式且已设置
- **WHEN** 供应商使用 `api_key_env = "ANTHROPIC_API_KEY"` 且该环境变量已设置
- **THEN** SHALL 显示 `✅ anthropic — ANTHROPIC_API_KEY is set`

#### Scenario: 环境变量方式但未设置
- **WHEN** 供应商使用 `api_key_env = "OPENAI_API_KEY"` 且该环境变量未设置
- **THEN** SHALL 显示 `❌ openai — OPENAI_API_KEY not set`

#### Scenario: 配置文件方式
- **WHEN** 供应商使用 `api_key = "sk-..."` 直接存储
- **THEN** SHALL 显示 `✅ openai — API key configured (config file)`

#### Scenario: 无 key 配置
- **WHEN** 供应商既没有 `api_key` 也没有 `api_key_env`
- **THEN** SHALL 显示 `❌ provider_name — no API key configured`

### Requirement: Agent 诊断
doctor SHALL 检查 merge 后每个 Agent 的供应商引用是否有效。

#### Scenario: Agent 引用有效供应商
- **WHEN** Agent 的 provider 字段指向已存在且 key 可用的供应商
- **THEN** SHALL 显示 `✅ agent_name — provider: provider_name ✓, model: model_name`

#### Scenario: Agent 引用不存在的供应商
- **WHEN** Agent 的 provider 字段指向不存在的供应商
- **THEN** SHALL 显示 `❌ agent_name — provider: provider_name (not found)`

#### Scenario: Agent 引用的供应商缺少 key
- **WHEN** Agent 的 provider 存在但 API key 未设置
- **THEN** SHALL 显示 `⚠️ agent_name — provider: provider_name (API key not set)`

### Requirement: MCP 服务器诊断
doctor SHALL 检查 MCP 服务器配置的基本有效性。

#### Scenario: stdio 类型 MCP 服务器
- **WHEN** MCP 服务器配置了 `command` 字段
- **THEN** SHALL 检查命令是否可在 PATH 中找到，显示 ✅ 或 ⚠️

#### Scenario: HTTP 类型 MCP 服务器
- **WHEN** MCP 服务器配置了 `url` 字段
- **THEN** SHALL 显示 `✅ server_name — url: <url>`（不做连通性测试）

#### Scenario: 无 MCP 服务器
- **WHEN** 配置中无 MCP 服务器
- **THEN** SHALL 跳过 MCP 诊断段落，不输出任何内容

### Requirement: 诊断总结
doctor SHALL 在末尾输出汇总信息。

#### Scenario: 全部健康
- **WHEN** 所有检查项均通过
- **THEN** SHALL 显示 "All checks passed. Configuration is ready."

#### Scenario: 部分问题
- **WHEN** 3 个 Agent 中有 2 个正常、1 个有问题
- **THEN** SHALL 显示 "Result: 2/3 agents available, 1 provider needs configuration"

#### Scenario: 无配置文件
- **WHEN** 两个配置文件都不存在
- **THEN** SHALL 显示 "No configuration files found. Run `krew config init` to get started."
