## ADDED Requirements

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

### Requirement: Frontmatter parsing
The system SHALL parse YAML frontmatter from command files. Frontmatter is delimited by `---` lines at the start of the file. Supported fields: `description` (string) and `argument-hint` (string). Both fields are optional.

#### Scenario: Full frontmatter
- **WHEN** a command file starts with `---\ndescription: Create a commit\nargument-hint: [message]\n---`
- **THEN** the system SHALL extract `description` as "Create a commit" and `argument-hint` as "[message]"

#### Scenario: Partial frontmatter
- **WHEN** a command file has frontmatter with only `description` field
- **THEN** the system SHALL extract `description` and use empty string for `argument-hint`

#### Scenario: No frontmatter
- **WHEN** a command file has no `---` delimiters at the start
- **THEN** the system SHALL use the entire file content as command body, with empty description and argument-hint

### Requirement: Argument substitution
The system SHALL replace argument placeholders in the command body before sending. `$ARGUMENTS` SHALL be replaced with the full argument string. `$1`, `$2`, etc. SHALL be replaced with positional arguments (whitespace-split). Unreferenced positional placeholders SHALL be replaced with empty string.

#### Scenario: $ARGUMENTS substitution
- **WHEN** user runs `/commit fix typo in readme` and command body contains `$ARGUMENTS`
- **THEN** `$ARGUMENTS` SHALL be replaced with "fix typo in readme"

#### Scenario: Positional arguments
- **WHEN** user runs `/deploy staging v2.0` and command body contains `$1` and `$2`
- **THEN** `$1` SHALL be replaced with "staging" and `$2` SHALL be replaced with "v2.0"

#### Scenario: Missing positional argument
- **WHEN** user runs `/deploy staging` and command body contains `$1` and `$2`
- **THEN** `$1` SHALL be replaced with "staging" and `$2` SHALL be replaced with empty string

#### Scenario: No arguments provided
- **WHEN** user runs `/commit` with no arguments and command body contains `$ARGUMENTS`
- **THEN** `$ARGUMENTS` SHALL be replaced with empty string

### Requirement: Command execution flow
After argument substitution and bash preprocessing, the expanded command text SHALL be routed through `parse_input()` for `@agent` addressing and sent as a normal user message.

#### Scenario: Command with @agent addressing
- **WHEN** a custom command body expands to `@coder review this code`
- **THEN** the system SHALL route the message to agent "coder" via normal `parse_input()` routing

#### Scenario: Command without @agent addressing
- **WHEN** a custom command body expands to `summarize the changes` (no @ prefix)
- **THEN** the system SHALL route the message to `Addressee::LastRespondent` (same as normal input)

### Requirement: Built-in command priority
Built-in slash commands SHALL always take priority over custom commands with the same name. Custom commands SHALL NOT be able to override built-in commands.

#### Scenario: Name collision with built-in
- **WHEN** both built-in `/help` and custom `.krew/commands/help.md` exist
- **THEN** `/help` SHALL execute the built-in command; the custom command SHALL be ignored

#### Scenario: No collision
- **WHEN** custom `/review` exists and no built-in `/review` exists
- **THEN** `/review` SHALL execute the custom command

### Requirement: Custom command in /help
The `/help` command SHALL display custom commands after built-in commands, with a "Custom commands:" subheading if any custom commands exist.

#### Scenario: Help with custom commands
- **WHEN** user runs `/help` and custom commands `/commit` (description: "Create a commit") and `/review:pr` exist
- **THEN** the output SHALL show built-in commands first, then a "Custom commands:" subheading, followed by custom command entries showing name and description

#### Scenario: Help with no custom commands
- **WHEN** user runs `/help` and no custom commands exist
- **THEN** the output SHALL show only built-in commands with no "Custom commands:" section
