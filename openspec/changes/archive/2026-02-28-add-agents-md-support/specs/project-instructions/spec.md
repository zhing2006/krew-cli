## ADDED Requirements

### Requirement: 指令文件发现
`krew-config` SHALL 提供 `load_project_instructions(cwd: &Path) -> Result<Option<String>>` 函数，从指定目录开始向上遍历父目录，查找所有名为 `AGENTS.md` 的文件。找到时返回合并后的内容字符串，未找到任何文件时返回 `None`。

#### Scenario: 工作目录存在 AGENTS.md
- **WHEN** 工作目录下存在 `AGENTS.md` 文件
- **THEN** 函数 SHALL 返回 `Some` 包含该文件的内容

#### Scenario: 工作目录无 AGENTS.md
- **WHEN** 工作目录及所有父目录均不存在 `AGENTS.md` 文件
- **THEN** 函数 SHALL 返回 `None`

#### Scenario: 非 UTF-8 编码文件
- **WHEN** `AGENTS.md` 文件不是有效的 UTF-8 编码
- **THEN** 函数 SHALL 跳过该文件并记录警告日志，继续查找其他层级

### Requirement: 层级化合并
当多个层级的目录中均存在 `AGENTS.md` 时，系统 SHALL 按从祖先到子目录的顺序合并内容，各文件内容之间以空行分隔。

#### Scenario: 多层级 AGENTS.md 合并
- **WHEN** `/project/AGENTS.md` 和 `/project/subdir/AGENTS.md` 均存在，且 cwd 为 `/project/subdir`
- **THEN** 合并结果 SHALL 先包含 `/project/AGENTS.md` 的内容，再包含 `/project/subdir/AGENTS.md` 的内容，两者之间以空行分隔

### Requirement: 文件大小限制
系统 SHALL 对单个 `AGENTS.md` 文件施加 100KB 的大小限制。

#### Scenario: 文件超过大小限制
- **WHEN** 某个 `AGENTS.md` 文件大小超过 100KB
- **THEN** 系统 SHALL 截断内容至 100KB，并在末尾追加 `\n\n[WARNING: File truncated at 100KB limit]`

### Requirement: 指令注入到系统提示词
`krew-core` 在构建发送给 LLM 的系统消息时，SHALL 将加载的项目指令内容注入到 Agent 的 `system_prompt` 之前。注入格式 SHALL 为：

```
<project-instructions>
{指令内容}
</project-instructions>

{agent system_prompt}
```

当项目指令为空（`None`）时，系统消息 SHALL 仅包含 `system_prompt` 原始内容。

#### Scenario: 有项目指令且有 system_prompt
- **WHEN** 项目指令内容为 "Use Rust conventions" 且 Agent 的 system_prompt 为 "You are a helpful assistant"
- **THEN** 最终系统消息 SHALL 为 `<project-instructions>\nUse Rust conventions\n</project-instructions>\n\nYou are a helpful assistant`

#### Scenario: 有项目指令但无 system_prompt
- **WHEN** 项目指令内容存在且 Agent 的 system_prompt 为 None 或空字符串
- **THEN** 最终系统消息 SHALL 仅包含 `<project-instructions>\n{内容}\n</project-instructions>`

#### Scenario: 无项目指令
- **WHEN** 项目指令为 None
- **THEN** 最终系统消息 SHALL 使用 Agent 的 system_prompt 原始值（可能为 None）
