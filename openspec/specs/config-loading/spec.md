## ADDED Requirements

### Requirement: Config::load() 从文件加载配置
`krew-config` SHALL 在 `Config` 上实现 `pub fn load(path: &Path) -> Result<Config, ConfigError>` 静态方法，读取指定路径的 TOML 文件并反序列化为 `Config` 结构体。

#### Scenario: 成功加载有效配置文件
- **WHEN** 调用 `Config::load()` 并传入一个有效的 `.krew/settings.toml` 路径
- **THEN** SHALL 返回 `Ok(Config)`，其中所有字段正确反序列化

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
- 一个名为 `"echo"` 的 agent（`display_name` = `"Echo"`，`provider` = `"builtin"`，`model` = `"echo"`，`color` = `"yellow"`，`tools` = `false`）
- 空的 `providers` HashMap
- 空的 `mcp_servers` Vec

#### Scenario: 默认配置可用
- **WHEN** 调用 `Config::default()`
- **THEN** SHALL 返回包含上述字段值的 `Config` 实例

#### Scenario: 默认配置中包含 echo agent
- **WHEN** 获取默认配置的 `agents` 列表
- **THEN** SHALL 包含恰好一个名为 `"echo"` 的 agent

### Requirement: 配置文件路径常量
`krew-config` SHALL 定义公开常量 `CONFIG_FILENAME`，值为 `".krew/settings.toml"`。

#### Scenario: 常量可访问
- **WHEN** 导入 `krew_config::CONFIG_FILENAME`
- **THEN** 其值 SHALL 为 `".krew/settings.toml"`
