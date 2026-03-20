## MODIFIED Requirements

### Requirement: Slash 命令补全
输入框第一行以 `/` 开头时，SHALL 自动显示补全弹窗，列出匹配的 Slash 命令。弹窗 SHALL 包含内置命令和自定义命令，自定义命令附加在内置命令之后。

#### Scenario: 输入 / 触发补全
- **WHEN** 用户在空输入框中输入 `/`
- **THEN** 弹窗 SHALL 显示所有内置 Slash 命令，后接所有自定义命令（名称 + 描述），最多显示 8 行

#### Scenario: 输入过滤包含自定义命令
- **WHEN** 用户输入 `/co` 且自定义命令 `/commit` 已注册
- **THEN** 弹窗 SHALL 显示匹配的内置命令（如 `/compact`）和自定义命令（如 `/commit`）

#### Scenario: 仅自定义命令匹配
- **WHEN** 用户输入 `/rev` 且自定义命令 `/review:pr` 已注册，无内置命令以 `rev` 开头
- **THEN** 弹窗 SHALL 只显示自定义命令 `/review:pr`

#### Scenario: 无匹配
- **WHEN** 用户输入 `/xyz`（内置和自定义命令均无匹配）
- **THEN** 弹窗 SHALL 关闭
