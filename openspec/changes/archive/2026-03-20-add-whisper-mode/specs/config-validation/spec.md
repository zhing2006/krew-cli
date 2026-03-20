## ADDED Requirements

### Requirement: Agent 名称保留字校验
配置校验 SHALL 禁止 `"all"` 作为 agent 名称。`"all"` 在 `@all`（广播寻址）和 `#all`（被禁止的全员密语）中均为保留字，将其用作 agent 名称会导致解析歧义。

#### Scenario: 配置包含 agent 名称 "all" 时报错
- **WHEN** 配置文件中存在一个 `name = "all"` 的 agent 定义
- **THEN** `validate()` SHALL 返回错误，错误消息 SHALL 说明 `"all"` 是保留字，不可用作 agent 名称

#### Scenario: 配置中无 "all" agent 时正常通过
- **WHEN** 配置文件中所有 agent 名称均非 `"all"`
- **THEN** `validate()` SHALL 不因 agent 名称校验而报错

#### Scenario: 大小写敏感
- **WHEN** 配置文件中存在 `name = "All"` 或 `name = "ALL"` 的 agent 定义
- **THEN** `validate()` SHALL 不拒绝该名称（保留字仅匹配全小写 `"all"`）
