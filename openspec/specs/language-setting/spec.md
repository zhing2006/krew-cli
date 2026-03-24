### Requirement: Settings 结构体包含 language 字段

`Settings` 结构体 SHALL 包含一个 `language` 字段，类型为 `Option<String>`，默认值为 `None`。

#### Scenario: 配置文件中指定 language
- **WHEN** 配置文件包含 `[settings]` 且设置了 `language = "中文"`
- **THEN** 解析后 `Settings.language` 的值 SHALL 为 `Some("中文")`

#### Scenario: 配置文件中未指定 language
- **WHEN** 配置文件的 `[settings]` 中没有 `language` 字段
- **THEN** 解析后 `Settings.language` 的值 SHALL 为 `None`

### Requirement: User 级和 Project 级 language 配置合并

`language` 字段 SHALL 遵循标准的 user/project 合并语义：project 级的值覆盖 user 级的值；如果 project 级未设置，则继承 user 级的值。

#### Scenario: Project 级覆盖 user 级
- **WHEN** user 级配置设置 `language = "English"` 且 project 级配置设置 `language = "中文"`
- **THEN** 合并后 `language` 的值 SHALL 为 `"中文"`

#### Scenario: Project 级未设置，继承 user 级
- **WHEN** user 级配置设置 `language = "日本語"` 且 project 级配置未设置 `language`
- **THEN** 合并后 `language` 的值 SHALL 为 `"日本語"`

### Requirement: System prompt 中注入语言指令

当 `Settings.language` 有值时，系统 SHALL 在基础 identity 块中（日期时间行之后、peer agent 协作提示和 whisper 上下文之前）注入语言指令。当 `Settings.language` 为 `None` 时，系统 SHALL NOT 注入任何语言指令。注入文本格式 MUST 为：

```
Always respond in {language}. Use {language} for all explanations, comments, and communications with the user. Technical terms and code identifiers should remain in their original form.
```

其中 `{language}` 替换为 `Settings.language` 的值。

#### Scenario: 语言指令出现在 identity 块中
- **WHEN** `Settings.language` 为 `"中文"` 且 Agent 开始生成回复
- **THEN** Agent 的 system prompt identity 块 SHALL 包含 `"Always respond in 中文. Use 中文 for all explanations, comments, and communications with the user. Technical terms and code identifiers should remain in their original form."`

#### Scenario: 未配置 language 时不注入
- **WHEN** 用户未配置 `language`（`Settings.language` 为 `None`）
- **THEN** Agent 的 system prompt identity 块 SHALL NOT 包含任何语言指令

### Requirement: AgentRuntime 传递 language 配置

`AgentRuntime` 结构体 SHALL 包含一个 `language: Option<String>` 字段，在初始化时从 `Settings.language` 获取。

#### Scenario: AgentRuntime 初始化时获取 language
- **WHEN** 系统初始化 Agent 运行时
- **THEN** `AgentRuntime.language` 的值 SHALL 等于 `Settings.language` 的值
