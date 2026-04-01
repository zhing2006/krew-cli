## ADDED Requirements

### Requirement: 危险路径常量定义
`krew-core` SHALL 定义以下硬编码保护清单常量，不可通过配置修改：

危险目录：`.git`、`.krew`、`.vscode`、`.idea`、`.claude`

危险文件：`.gitconfig`、`.gitmodules`、`.bashrc`、`.bash_profile`、`.zshrc`、`.zprofile`、`.profile`、`.env`

#### Scenario: 常量可访问
- **WHEN** 导入保护路径常量
- **THEN** SHALL 包含上述所有目录和文件名

### Requirement: 危险路径检查函数
`krew-core` SHALL 实现 `is_dangerous_path(file_path: &str) -> bool` 函数，检查文件路径是否在保护清单中。检查逻辑：
- 对 `DANGEROUS_DIRECTORIES`：文件路径的任意路径段匹配目录名
- 对 `DANGEROUS_FILES`：文件路径的文件名部分匹配文件名
- 匹配 SHALL 不区分大小写（适配 Windows 文件系统）
- Windows 路径（反斜杠）SHALL 先转为 POSIX 格式再检查

#### Scenario: .git 目录下的文件
- **WHEN** 检查路径 `.git/config`
- **THEN** SHALL 返回 `true`

#### Scenario: .git 深层路径
- **WHEN** 检查路径 `.git/refs/heads/main`
- **THEN** SHALL 返回 `true`

#### Scenario: .krew 配置文件
- **WHEN** 检查路径 `.krew/settings.toml`
- **THEN** SHALL 返回 `true`

#### Scenario: 危险文件名
- **WHEN** 检查路径 `.bashrc`
- **THEN** SHALL 返回 `true`

#### Scenario: .env 文件
- **WHEN** 检查路径 `.env`
- **THEN** SHALL 返回 `true`

#### Scenario: 正常文件不触发
- **WHEN** 检查路径 `src/main.rs`
- **THEN** SHALL 返回 `false`

#### Scenario: Windows 路径
- **WHEN** 检查路径 `.git\config`（Windows 反斜杠）
- **THEN** SHALL 返回 `true`

#### Scenario: 不区分大小写
- **WHEN** 检查路径 `.Git/config`
- **THEN** SHALL 返回 `true`

### Requirement: bypass 免疫审批行为
当 `write_file`、`edit_file` 或 `read_file` 工具的目标路径触发危险路径检查时，`check_tool_approval()` SHALL 在所有其他检查之前返回 `NeedsApproval`，包括：
- FullAuto 模式下仍然需要确认
- deny 规则之前执行（bypass 免疫优先级最高）
- 不受 session 缓存影响

#### Scenario: FullAuto 模式下保护 .git
- **WHEN** ApprovalMode 为 FullAuto 且工具调用为 `write_file` 目标为 `.git/hooks/pre-commit`
- **THEN** SHALL 返回 `NeedsApproval`（不自动放行）

#### Scenario: FullAuto 模式下保护 .krew
- **WHEN** ApprovalMode 为 FullAuto 且工具调用为 `edit_file` 目标为 `.krew/settings.toml`
- **THEN** SHALL 返回 `NeedsApproval`（不自动放行）

#### Scenario: read_file 读取 .env 时 FullAuto 仍需确认
- **WHEN** ApprovalMode 为 FullAuto 且工具调用为 `read_file` 目标为 `.env`
- **THEN** SHALL 返回 `NeedsApproval`（不自动放行）

#### Scenario: session 缓存不绕过保护
- **WHEN** 用户已通过 ApprovedForSession 批准了 `edit_file`
- **AND** 后续调用 `edit_file` 目标为 `.bashrc`
- **THEN** SHALL 仍然返回 `NeedsApproval`（保护路径不受缓存影响）

#### Scenario: 正常路径不受影响
- **WHEN** ApprovalMode 为 FullAuto 且工具调用为 `write_file` 目标为 `src/main.rs`
- **THEN** SHALL 返回 `Auto`（正常路径走常规流程）

### Requirement: Shell 内置危险模式 deny
`krew-core` SHALL 定义一组硬编码的 shell 危险模式常量，用于拦截通过 shell 修改保护路径的常见操作。这些模式作为不可覆盖的内置 deny 规则，在用户配置的 deny_rules 之前评估。

内置危险模式 SHALL 至少覆盖以下场景：
- 删除保护目录/文件的命令（如 `rm` 针对 `.git`、`.krew`、`.env` 等）
- 通过重定向写入保护路径的命令

匹配逻辑 SHALL 复用 `shell_parse.rs` 的命令段拆分，对每个命令段独立检查。

**局限性**：Shell 的保护只能覆盖常见模式，无法穷举所有可能修改保护路径的 shell 命令（变量展开、子 shell、管道组合等）。FullAuto 模式下 shell 访问具有固有风险。

#### Scenario: Shell 删除 .git 被拦截
- **WHEN** ApprovalMode 为 FullAuto 且 shell 命令为 `rm -rf .git`
- **THEN** SHALL 返回 `Denied`（内置 deny，不可覆盖）

#### Scenario: Shell 删除 .krew 被拦截
- **WHEN** ApprovalMode 为 FullAuto 且 shell 命令为 `rm -rf .krew/`
- **THEN** SHALL 返回 `Denied`（内置 deny，不可覆盖）

#### Scenario: Shell 删除 .env 被拦截
- **WHEN** ApprovalMode 为 FullAuto 且 shell 命令为 `rm .env`
- **THEN** SHALL 返回 `Denied`（内置 deny，不可覆盖）

#### Scenario: Shell 正常命令不受影响
- **WHEN** ApprovalMode 为 FullAuto 且 shell 命令为 `rm -rf target/`
- **THEN** SHALL 不被内置 deny 拦截（走正常审批流程）
