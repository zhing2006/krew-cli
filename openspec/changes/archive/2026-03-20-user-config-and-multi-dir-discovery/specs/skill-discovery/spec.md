## MODIFIED Requirements

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

#### Scenario: .agents 优先于 .claude
- **WHEN** `<cwd>/.agents/skills/foo/SKILL.md` 和 `<cwd>/.claude/skills/foo/SKILL.md` 均存在
- **THEN** 系统 SHALL 使用 `.agents/skills/` 中的版本

#### Scenario: 额外路径
- **WHEN** `extra_paths` 包含 `/opt/skills/` 且其下有 `my-skill/SKILL.md`
- **THEN** 函数 SHALL 返回包含该 skill 的 `SkillRecord`
