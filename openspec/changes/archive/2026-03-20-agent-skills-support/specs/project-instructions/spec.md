## MODIFIED Requirements

### Requirement: 指令注入到系统提示词
`krew-core` 在构建发送给 LLM 的系统消息时，SHALL 将加载的项目指令内容注入到 Agent 的 `system_prompt` 之前。当有可用 skills 时，skill catalog SHALL 插入在项目指令和 agent prompt 之间。注入格式 SHALL 为：

```
<project-instructions>
{指令内容}
</project-instructions>

{skill catalog + 行为指令（仅当有可用 skills 时）}

{agent system_prompt}
```

当项目指令为空（`None`）时，系统消息 SHALL 以 skill catalog（如有）开头，然后是 `system_prompt`。当 skill catalog 也为空时，行为与现有逻辑一致。

#### Scenario: 有项目指令、有 skills、有 system_prompt
- **WHEN** 项目指令内容为 "Use Rust conventions"，有 2 个 skills，且 Agent 的 system_prompt 为 "You are a helpful assistant"
- **THEN** 最终系统消息 SHALL 按顺序包含：project-instructions 标签 → skill catalog XML + 行为指令 → system_prompt

#### Scenario: 有项目指令但无 skills
- **WHEN** 项目指令存在但没有可用 skills
- **THEN** 最终系统消息 SHALL 与现有行为一致：`<project-instructions>` 标签 + system_prompt

#### Scenario: 无项目指令但有 skills
- **WHEN** 项目指令为 None 但有可用 skills
- **THEN** 最终系统消息 SHALL 以 skill catalog 开头，然后是 agent system_prompt
