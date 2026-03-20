## ADDED Requirements

### Requirement: Skill 目录扫描
`krew-core` SHALL 提供 `discover_skills(cwd: &Path, extra_paths: &[PathBuf]) -> Vec<SkillRecord>` 函数，按优先级顺序扫描以下路径查找包含 `SKILL.md` 文件的子目录：
1. `<cwd>/.krew/skills/`
2. `<cwd>/.agents/skills/`
3. `<cwd>/.claude/skills/`
4. `<home>/.krew/skills/`
5. `<home>/.agents/skills/`
6. `<home>/.claude/skills/`
7. `extra_paths` 中的每个路径

扫描时 SHALL 跳过 `.git/`、`node_modules/`、`target/` 目录。扫描深度 SHALL 限制为 4 层。不存在的目录 SHALL 静默跳过。

#### Scenario: 项目级 skill 发现
- **WHEN** `<cwd>/.krew/skills/code-review/SKILL.md` 存在
- **THEN** 函数 SHALL 返回包含该 skill 的 `SkillRecord`

#### Scenario: .claude 目录 skill 发现
- **WHEN** `<cwd>/.claude/skills/data-analysis/SKILL.md` 存在（且 `.krew/` 和 `.agents/` 中无同名 skill）
- **THEN** 函数 SHALL 返回包含该 skill 的 `SkillRecord`

#### Scenario: 用户级 skill 发现
- **WHEN** `<home>/.agents/skills/data-analysis/SKILL.md` 存在
- **THEN** 函数 SHALL 返回包含该 skill 的 `SkillRecord`

#### Scenario: 用户级 .claude skill 发现
- **WHEN** `<home>/.claude/skills/my-skill/SKILL.md` 存在（且其他路径中无同名 skill）
- **THEN** 函数 SHALL 返回包含该 skill 的 `SkillRecord`

#### Scenario: 目录不存在
- **WHEN** `<cwd>/.claude/skills/` 目录不存在
- **THEN** 函数 SHALL 静默跳过该路径，不报错

#### Scenario: 跳过排除目录
- **WHEN** skills 目录下存在 `.git/` 或 `node_modules/` 子目录
- **THEN** 扫描 SHALL 跳过这些目录

#### Scenario: .agents 优先于 .claude
- **WHEN** `<cwd>/.agents/skills/foo/SKILL.md` 和 `<cwd>/.claude/skills/foo/SKILL.md` 均存在
- **THEN** 系统 SHALL 使用 `.agents/skills/` 中的版本

#### Scenario: 额外路径
- **WHEN** `extra_paths` 包含 `/opt/skills/` 且其下有 `my-skill/SKILL.md`
- **THEN** 函数 SHALL 返回包含该 skill 的 `SkillRecord`

### Requirement: SKILL.md 解析
`krew-core` SHALL 提供 `parse_skill_md(path: &Path) -> Result<SkillRecord, SkillError>` 函数，解析 `SKILL.md` 文件的 YAML frontmatter 和 Markdown body。YAML frontmatter SHALL 以 `---` 行开始和结束。

#### Scenario: 合法的 SKILL.md
- **WHEN** 文件包含有效的 frontmatter（name + description）和 Markdown body
- **THEN** 函数 SHALL 返回 `Ok(SkillRecord)` 包含所有解析出的字段

#### Scenario: 缺少 description
- **WHEN** frontmatter 中没有 `description` 字段或值为空
- **THEN** 函数 SHALL 返回 `Err(SkillError)` 指示缺少必需字段

#### Scenario: YAML 不可解析
- **WHEN** frontmatter 中的 YAML 语法完全无效
- **THEN** 函数 SHALL 返回 `Err(SkillError)` 包含解析错误信息

#### Scenario: name 不匹配目录名
- **WHEN** frontmatter 中 `name` 与父目录名不一致
- **THEN** 函数 SHALL 记录警告日志并仍返回 `Ok(SkillRecord)`

#### Scenario: 可选字段解析
- **WHEN** frontmatter 包含 `license`、`compatibility`、`metadata` 等可选字段
- **THEN** 函数 SHALL 正确解析并存储到 `SkillRecord` 中

### Requirement: 名称冲突处理
当多个 skill 具有相同 `name` 时，系统 SHALL 使用先发现的 skill（即优先级更高路径中的 skill），并记录警告日志指出被覆盖的 skill 路径。

#### Scenario: 项目级覆盖用户级
- **WHEN** `<cwd>/.krew/skills/code-review/SKILL.md` 和 `<home>/.agents/skills/code-review/SKILL.md` 均存在
- **THEN** 系统 SHALL 使用项目级的 skill，并记录警告日志

#### Scenario: 同范围内冲突
- **WHEN** `<cwd>/.krew/skills/foo/SKILL.md` 和 `<cwd>/.agents/skills/foo/SKILL.md` 均存在
- **THEN** 系统 SHALL 使用 `.krew/skills/` 中的（优先级 1），因为它先被扫描

### Requirement: SkillRecord 数据结构
`krew-core` SHALL 定义 `SkillRecord` 结构体，包含字段：`name: String`、`description: String`、`location: PathBuf`（SKILL.md 的绝对路径）、`base_dir: PathBuf`（skill 目录的绝对路径）、`compatibility: Option<String>`、`metadata: Option<HashMap<String, String>>`。

#### Scenario: SkillRecord 字段完整
- **WHEN** 构造一个 `SkillRecord`
- **THEN** 所有指定字段 SHALL 存在且类型正确

### Requirement: SkillError 错误类型
`krew-core` SHALL 定义 `SkillError` 枚举，包含变体：`ParseError(String)`（frontmatter 解析失败）、`MissingField(String)`（缺少必需字段）、`IoError(std::io::Error)`（文件读取失败）。

#### Scenario: 错误类型变体
- **WHEN** 创建各种 SkillError 变体
- **THEN** 各变体 SHALL 包含有意义的错误信息
