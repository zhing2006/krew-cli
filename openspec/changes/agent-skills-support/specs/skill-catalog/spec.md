## ADDED Requirements

### Requirement: Skill catalog 构建
`krew-core` SHALL 提供 `build_skill_catalog(skills: &[SkillRecord]) -> String` 函数，根据已发现的 skills 构建 XML 格式的 catalog 文本。格式 SHALL 为：

```xml
<available-skills>
  <skill name="{name}" location="{SKILL.md absolute path}">
    {description}
  </skill>
  ...
</available-skills>
```

#### Scenario: 多个 skills
- **WHEN** 传入包含 3 个 SkillRecord 的切片
- **THEN** 函数 SHALL 返回包含 3 个 `<skill>` 元素的 XML 文本

#### Scenario: 空 skills 列表
- **WHEN** 传入空切片
- **THEN** 函数 SHALL 返回空字符串

### Requirement: Catalog 注入到 system prompt
`krew-core` 在构建 agent 的 system prompt 时，SHALL 在 agent identity 块之前注入 skill catalog 和行为指令。注入格式 SHALL 为：

```
{skill_catalog}

The following skills provide specialized instructions for specific tasks.
When a task matches a skill's description, call the activate_skill tool
with the skill's name to load its full instructions.
When a skill references relative paths, resolve them against the skill's
directory and use the read_file tool with absolute paths.
```

当没有可用 skills 时（catalog 为空），SHALL 不注入任何 skill 相关内容。

#### Scenario: 有可用 skills
- **WHEN** 系统发现了 2 个 skills
- **THEN** agent 的 system prompt SHALL 包含 skill catalog XML 和行为指令

#### Scenario: 无可用 skills
- **WHEN** 系统未发现任何 skill
- **THEN** agent 的 system prompt SHALL 不包含任何 skill 相关内容

### Requirement: System prompt 层级顺序
注入 skill catalog 后，完整 system prompt 的层级顺序 SHALL 为：
1. `<project-instructions>` （来自 AGENTS.md）
2. Skill catalog + 行为指令
3. Agent identity 块 + agent 自定义 system_prompt

#### Scenario: 完整 system prompt 构建
- **WHEN** 存在项目指令、2 个 skills、和 agent system_prompt
- **THEN** 最终 system prompt SHALL 按 project-instructions → skill-catalog → agent-prompt 的顺序排列
