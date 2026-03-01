## ADDED Requirements

### Requirement: Config::validate() 校验方法
`krew-config` SHALL 在 `Config` 上实现 `pub fn validate(&self) -> Result<(), ConfigError>` 方法，检查配置的内部一致性。

#### Scenario: 有效配置通过校验
- **WHEN** 调用 `validate()` 且配置中所有引用均合法
- **THEN** SHALL 返回 `Ok(())`

#### Scenario: reply_order 引用不存在的 agent
- **WHEN** `settings.reply_order` 包含一个不在 `agents` 列表中的名称
- **THEN** SHALL 返回 `Err(ConfigError::Validation(...))` 且错误信息包含该无效名称

#### Scenario: agent 引用不存在的 provider
- **WHEN** 某个 agent 的 `provider` 字段引用了不在 `providers` 中的名称
- **THEN** SHALL 返回 `Err(ConfigError::Validation(...))` 且错误信息包含 agent 名称和无效 provider 名称

#### Scenario: 重复的 agent name
- **WHEN** `agents` 列表中有两个或以上 agent 使用相同的 `name`
- **THEN** SHALL 返回 `Err(ConfigError::Validation(...))` 且错误信息包含重复的名称

### Requirement: 内置 provider 跳过引用检查
`Config::validate()` SHALL 对 `provider` 值为 `"builtin"` 的 agent 跳过 provider 引用检查，因为内置 provider 不需要在 `providers` 表中定义。

#### Scenario: builtin provider 不报错
- **WHEN** 一个 agent 的 `provider` 为 `"builtin"` 且 `providers` 中不存在 `"builtin"` 键
- **THEN** SHALL 通过校验而不报错
