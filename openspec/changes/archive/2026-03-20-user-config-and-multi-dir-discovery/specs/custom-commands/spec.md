## MODIFIED Requirements

### Requirement: Command file discovery
系统 SHALL 在启动时扫描多个目录查找 `.md` 文件并注册为自定义 slash 命令。扫描路径按优先级从高到低为：
1. `<cwd>/.krew/commands/`
2. `<cwd>/.agents/commands/`
3. `<cwd>/.claude/commands/`
4. `<home>/.krew/commands/`
5. `<home>/.agents/commands/`
6. `<home>/.claude/commands/`

子目录 SHALL 递归扫描。同名命令 SHALL 使用优先级最高路径中的版本（first-found wins）。

#### Scenario: .krew 目录中的命令
- **WHEN** `.krew/commands/commit.md` 存在
- **THEN** 系统 SHALL 注册为自定义命令 `/commit`

#### Scenario: .agents 目录中的命令
- **WHEN** `.agents/commands/review.md` 存在（且 `.krew/commands/review.md` 不存在）
- **THEN** 系统 SHALL 注册为自定义命令 `/review`

#### Scenario: .claude 目录中的命令
- **WHEN** `.claude/commands/deploy.md` 存在（且 `.krew/` 和 `.agents/` 中无同名文件）
- **THEN** 系统 SHALL 注册为自定义命令 `/deploy`

#### Scenario: User-level 命令
- **WHEN** `~/.krew/commands/global-lint.md` 存在（且 project 目录中无同名文件）
- **THEN** 系统 SHALL 注册为自定义命令 `/global-lint`

#### Scenario: 同名命令优先级
- **WHEN** `.krew/commands/commit.md` 和 `.claude/commands/commit.md` 同时存在
- **THEN** 系统 SHALL 使用 `.krew/commands/commit.md`，忽略 `.claude/` 中的版本

#### Scenario: Nested 命令文件
- **WHEN** `.agents/commands/git/push.md` 存在
- **THEN** 系统 SHALL 注册为自定义命令 `/git:push`

#### Scenario: 无任何命令目录
- **WHEN** 所有 6 个目录均不存在
- **THEN** 系统 SHALL 正常启动，自定义命令注册表为空

#### Scenario: Non-md 文件忽略
- **WHEN** 命令目录中包含无 `.md` 扩展名的文件
- **THEN** 系统 SHALL 忽略这些文件

### Requirement: discover_commands 函数签名
`discover_commands` 函数签名 SHALL 改为 `pub fn discover_commands(cwd: &Path) -> CustomCommandRegistry`，接受 cwd 参数并扫描所有 discovery 路径。

#### Scenario: 函数签名
- **WHEN** 调用 `discover_commands(cwd)`
- **THEN** SHALL 返回包含从所有 discovery 路径发现的命令的 `CustomCommandRegistry`
