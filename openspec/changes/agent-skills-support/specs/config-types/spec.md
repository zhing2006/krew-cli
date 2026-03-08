## MODIFIED Requirements

### Requirement: Config 根结构体
`krew-config` SHALL 定义 `Config` 结构体，包含字段：`settings: Settings`、`agents: Vec<AgentConfig>`、`providers: HashMap<String, ProviderConfig>`、`mcp_servers: Vec<McpServerConfig>`、`skills: SkillsConfig`。该结构体 SHALL 派生 `Deserialize`。`skills` 字段 SHALL 使用 `Default` trait 提供默认值，当 TOML 中不存在 `[skills]` 节时自动使用默认配置。

#### Scenario: Config 结构体可导入
- **WHEN** 导入 `krew_config::Config`
- **THEN** 该类型 SHALL 可访问并包含所有指定字段，包括 `skills: SkillsConfig`

#### Scenario: skills 字段默认值
- **WHEN** TOML 配置中不包含 `[skills]` 节
- **THEN** `Config::skills` SHALL 使用 `SkillsConfig::default()`（enabled=true, extra_paths=[]）
