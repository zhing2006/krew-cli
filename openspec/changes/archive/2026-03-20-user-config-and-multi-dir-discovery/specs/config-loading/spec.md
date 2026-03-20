## MODIFIED Requirements

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

## ADDED Requirements

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
3. 调用 `raw.merge_user(&user_config)` 合并 user 配置
4. 调用 `raw.resolve()` 填充默认值生成最终 `Config`
5. 调用 `config.apply_cli_overrides()` 应用 CLI 参数覆盖
6. 调用 `config.validate()` 校验合并后的配置

validate() SHALL 在 apply_cli_overrides() **之后**执行，保持与现有行为一致。

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
