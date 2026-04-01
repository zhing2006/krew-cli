## ADDED Requirements

### Requirement: Config::load() 从文件加载配置
`krew-config` SHALL 在 `Config` 上保留 `pub fn load(path: &Path) -> Result<Config, ConfigError>` 静态方法。该方法 SHALL 直接反序列化为 `Config`（严格模式：缺少 `agents`、`settings` 等必需字段时返回 parse 错误）。对于需要 user+project 合并的场景，SHALL 使用 `RawConfig::load()` 路径。

#### Scenario: 成功加载有效配置文件
- **WHEN** 调用 `Config::load()` 并传入一个有效的 `.krew/settings.toml` 路径
- **THEN** SHALL 返回 `Ok(Config)`，其中所有字段正确反序列化

#### Scenario: 缺少必需字段
- **WHEN** 调用 `Config::load()` 并传入缺少 `[[agents]]` 或 `[settings]` 的 TOML 文件
- **THEN** SHALL 返回 `Err(ConfigError::Parse(...))`

#### Scenario: 文件不存在
- **WHEN** 调用 `Config::load()` 并传入一个不存在的路径
- **THEN** SHALL 返回 `Err(ConfigError::Io(...))`

#### Scenario: TOML 格式错误
- **WHEN** 调用 `Config::load()` 并传入一个内容格式错误的文件
- **THEN** SHALL 返回 `Err(ConfigError::Parse(...))`，错误信息包含行号和原因

### Requirement: ConfigError 错误类型
`krew-config` SHALL 定义 `ConfigError` 枚举，使用 `thiserror` 派生 `Error`，包含以下变体：
- `Io(std::io::Error)` — 文件 IO 错误
- `Parse(toml::de::Error)` — TOML 解析/反序列化错误
- `Validation(String)` — 配置校验错误

#### Scenario: IO 错误转换
- **WHEN** 文件读取失败返回 `std::io::Error`
- **THEN** SHALL 可通过 `?` 运算符自动转换为 `ConfigError::Io`

#### Scenario: 解析错误转换
- **WHEN** TOML 反序列化失败返回 `toml::de::Error`
- **THEN** SHALL 可通过 `?` 运算符自动转换为 `ConfigError::Parse`

### Requirement: Config 内置默认值
`krew-config` SHALL 为 `Config` 实现 `Default` trait，返回可直接运行的最小配置。默认配置 SHALL 包含：
- `settings.approval_mode` = `Suggest`
- `settings.reply_order` = `["echo"]`
- `settings.auto_compact_threshold` = `None`
- `settings.allow_rules` = `[]`（空 Vec）
- `settings.deny_rules` = `[]`（空 Vec）
- `settings.ask_rules` = `[]`（空 Vec）
- 一个名为 `"echo"` 的 agent（`display_name` = `"Echo"`，`provider` = `"builtin"`，`model` = `"echo"`，`color` = `"yellow"`，`tools` = `false`）
- 空的 `providers` HashMap
- 空的 `mcp_servers` Vec
- 不再包含 `settings.shell_allow_commands` 和 `settings.fetch_allow_domains` 字段

#### Scenario: 默认配置可用
- **WHEN** 调用 `Config::default()`
- **THEN** SHALL 返回包含上述字段值的 `Config` 实例

#### Scenario: 默认配置中包含 echo agent
- **WHEN** 获取默认配置的 `agents` 列表
- **THEN** SHALL 包含恰好一个名为 `"echo"` 的 agent

#### Scenario: 默认配置无规则
- **WHEN** 调用 `Config::default()`
- **THEN** `settings.allow_rules`、`settings.deny_rules`、`settings.ask_rules` SHALL 均为空 Vec

### Requirement: 配置文件路径常量
`krew-config` SHALL 定义公开常量 `CONFIG_FILENAME`，值为 `".krew/settings.toml"`。

#### Scenario: 常量可访问
- **WHEN** 导入 `krew_config::CONFIG_FILENAME`
- **THEN** 其值 SHALL 为 `".krew/settings.toml"`

### Requirement: RawConfig::load() 加载原始配置
`krew-config` SHALL 在 `RawConfig` 上实现 `pub fn load(path: &Path) -> Result<RawConfig, ConfigError>` 静态方法，读取 TOML 文件并反序列化为 `RawConfig`（保留字段存在性信息，不填充默认值）。

#### Scenario: 成功加载
- **WHEN** 调用 `RawConfig::load()` 并传入有效 TOML 文件
- **THEN** SHALL 返回 `Ok(RawConfig)`，未指定的 settings 字段为 `None`

#### Scenario: 文件不存在
- **WHEN** 调用 `RawConfig::load()` 并传入不存在的路径
- **THEN** SHALL 返回 `Err(ConfigError::Io(...))`

### Requirement: 启动时加载流程
`krew-cli` 的 `load_config()` 函数 SHALL 按以下顺序执行：
1. 调用 `UserConfig::load()` 加载 user-level 配置
2. 调用 `RawConfig::load()` 加载 project-level 配置（不存在时使用 `RawConfig::default()`）
3. 调用 `raw.merge_user(&user_config)` 合并 user 配置（包括合并 `allow_rules`、`deny_rules`、`ask_rules` 列表，两个来源的规则拼接，不去重）
4. 调用 `raw.resolve()` 填充默认值生成最终 `Config`
5. 调用 `config.apply_cli_overrides()` 应用 CLI 参数覆盖
6. 调用 `config.validate()` 校验合并后的配置

validate() SHALL 在 apply_cli_overrides() **之后**执行，保持与现有行为一致。

#### Scenario: 合并 user 和 project 规则
- **WHEN** user config 包含 `[[deny_rules]] tool = "shell" pattern = "rm *"` 且 project config 包含 `[[deny_rules]] tool = "shell" pattern = "dd *"`
- **THEN** 合并后的 `deny_rules` SHALL 包含两条规则（两个来源的规则拼接，不去重）

#### Scenario: 完整加载流程
- **WHEN** user config 和 project config 均存在
- **THEN** 系统 SHALL 按上述 6 步顺序执行，最终产生合并后的 `Config`

#### Scenario: 仅 project config 存在
- **WHEN** user config 不存在，project config 存在
- **THEN** 系统 SHALL 使用 `UserConfig::default()` 参与合并，结果等同于仅 project config

#### Scenario: 均不存在
- **WHEN** user config 和 project config 均不存在
- **THEN** 系统 SHALL 使用 `RawConfig::default()` 和 `UserConfig::default()`，经 resolve 产生内置默认配置

#### Scenario: --config PATH 仍加载 user config
- **WHEN** 用户通过 `--config` 指定了自定义 project config 路径
- **THEN** 系统 SHALL 仍然加载 `~/.krew/settings.toml` 作为 user config 参与合并

#### Scenario: validate 在 CLI overrides 之后
- **WHEN** project config 中某 agent 引用了不存在的 provider，但用户通过 `--agents` 过滤掉了该 agent
- **THEN** validate SHALL 通过（因为 apply_cli_overrides 已移除了该 agent）
