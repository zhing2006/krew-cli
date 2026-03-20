## ADDED Requirements

### Requirement: activate_skill 工具定义
`krew-tools` SHALL 提供 `ActivateSkillTool` 实现 `ToolHandler` trait。该工具 SHALL 接受参数 `name: String`（必填，skill 名称）。工具名称 SHALL 为 `"activate_skill"`。

#### Scenario: 工具注册
- **WHEN** 系统初始化时有可用 skills
- **THEN** `activate_skill` 工具 SHALL 被注册到 `ToolRegistry` 中

#### Scenario: 无可用 skills
- **WHEN** 系统初始化时没有发现任何 skill
- **THEN** `activate_skill` 工具 SHALL 不被注册

### Requirement: activate_skill 工具为只读
`ActivateSkillTool` 的 `requires_approval()` SHALL 返回 `false`，使其作为只读工具自动执行，无需用户审批。

#### Scenario: 无需审批
- **WHEN** LLM 调用 `activate_skill` 工具
- **THEN** 系统 SHALL 自动执行，不提示用户审批

### Requirement: activate_skill 返回内容
当 LLM 调用 `activate_skill` 传入合法的 skill name 时，工具 SHALL 读取对应 `SKILL.md` 文件，剥离 YAML frontmatter，返回以下结构化内容：

```xml
<skill_content name="{name}">
{SKILL.md body content}

Skill directory: {base_dir absolute path}
Relative paths in this skill are relative to the skill directory.

<skill_resources>
  <file>{relative path}</file>
  ...
</skill_resources>
</skill_content>
```

`<skill_resources>` SHALL 列出 skill 目录下 `scripts/`、`references/`、`assets/` 中的文件（若存在），但 SHALL 不读取这些文件的内容。

#### Scenario: 成功激活
- **WHEN** 调用 `activate_skill` 传入 `{ "name": "code-review" }` 且该 skill 存在
- **THEN** SHALL 返回 `ToolResult { is_error: false }` 包含 XML 包装的 skill 内容和资源列表

#### Scenario: skill 不存在
- **WHEN** 调用 `activate_skill` 传入不存在的 skill name
- **THEN** SHALL 返回 `ToolResult { is_error: true }` 包含错误信息和可用 skill 名称列表

#### Scenario: SKILL.md 读取失败
- **WHEN** skill 的 SKILL.md 文件在激活时无法读取（如权限问题）
- **THEN** SHALL 返回 `ToolResult { is_error: true }` 包含 IO 错误信息

### Requirement: 资源文件枚举
`activate_skill` 工具在返回内容时 SHALL 枚举 skill 目录下 `scripts/`、`references/`、`assets/` 子目录中的文件，以相对路径形式列出。枚举深度 SHALL 限制为 2 层。空目录或不存在的子目录 SHALL 不在 `<skill_resources>` 中出现。

#### Scenario: 包含 scripts 的 skill
- **WHEN** skill 目录下有 `scripts/extract.py` 和 `scripts/merge.py`
- **THEN** 返回内容的 `<skill_resources>` SHALL 包含 `<file>scripts/extract.py</file>` 和 `<file>scripts/merge.py</file>`

#### Scenario: 无资源文件
- **WHEN** skill 目录下没有 `scripts/`、`references/`、`assets/` 子目录
- **THEN** 返回内容 SHALL 不包含 `<skill_resources>` 标签

### Requirement: 激活去重
系统 SHALL 跟踪当前会话中已激活的 skill 名称。当 LLM 再次尝试激活已激活的 skill 时，工具 SHALL 返回提示信息说明该 skill 已在上下文中，而非重复注入内容。

#### Scenario: 重复激活
- **WHEN** LLM 在同一会话中第二次调用 `activate_skill` 传入 `{ "name": "code-review" }`
- **THEN** SHALL 返回 `ToolResult { is_error: false }` 包含提示 "Skill 'code-review' is already activated in this session."

#### Scenario: 新会话重置
- **WHEN** 用户执行 `/new` 创建新会话
- **THEN** 已激活 skill 的跟踪记录 SHALL 被清空
