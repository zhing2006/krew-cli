## ADDED Requirements

### Requirement: 统一 discovery 路径生成
`krew-core` SHALL 提供 `pub fn discovery_paths(cwd: &Path, subdir: &str) -> Vec<PathBuf>` 函数，按优先级从高到低返回以下路径：
1. `<cwd>/.krew/<subdir>/`
2. `<cwd>/.agents/<subdir>/`
3. `<cwd>/.claude/<subdir>/`
4. `<home>/.krew/<subdir>/`
5. `<home>/.agents/<subdir>/`
6. `<home>/.claude/<subdir>/`

若 home 目录不可用，SHALL 仅返回前 3 个 project-level 路径。

#### Scenario: 完整路径列表
- **WHEN** 调用 `discovery_paths("/project", "commands")` 且 home 目录可用
- **THEN** SHALL 返回 6 个路径，按上述优先级排列

#### Scenario: home 不可用
- **WHEN** 调用 `discovery_paths("/project", "commands")` 且 HOME/USERPROFILE 环境变量均未设置
- **THEN** SHALL 返回 3 个 project-level 路径

#### Scenario: skills 子目录
- **WHEN** 调用 `discovery_paths("/project", "skills")`
- **THEN** 第一个路径 SHALL 为 `/project/.krew/skills/`

### Requirement: 优先级为 first-found wins
当 commands 或 skills 在多个 discovery 路径中发现同名条目时，系统 SHALL 使用优先级最高（最先扫描）的路径中的条目，后续同名条目 SHALL 被忽略。

#### Scenario: project .krew 优先于 project .agents
- **WHEN** `.krew/commands/deploy.md` 和 `.agents/commands/deploy.md` 同时存在
- **THEN** 系统 SHALL 使用 `.krew/commands/deploy.md`

#### Scenario: project .agents 优先于 project .claude
- **WHEN** `.agents/commands/review.md` 和 `.claude/commands/review.md` 同时存在
- **THEN** 系统 SHALL 使用 `.agents/commands/review.md`

#### Scenario: project level 优先于 user level
- **WHEN** `.krew/commands/commit.md` 和 `~/.krew/commands/commit.md` 同时存在
- **THEN** 系统 SHALL 使用 project level 的 `.krew/commands/commit.md`

#### Scenario: user level .krew 优先于 user level .claude
- **WHEN** `~/.krew/commands/lint.md` 和 `~/.claude/commands/lint.md` 同时存在
- **THEN** 系统 SHALL 使用 `~/.krew/commands/lint.md`
