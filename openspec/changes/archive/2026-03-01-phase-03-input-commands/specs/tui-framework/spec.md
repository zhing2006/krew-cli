## MODIFIED Requirements

### Requirement: 输入区域
输入区域 SHALL 显示 `› ` 绿色提示符，支持用户输入文本。输入区域上方有一条灰色分隔线，下方为分隔线和状态栏（或补全弹窗）。输入处理流程 SHALL 区分 Slash 命令和普通消息：Slash 命令直接执行，普通消息经过 @ 寻址解析后处理。

#### Scenario: 输入框可见
- **WHEN** TUI 界面渲染完成
- **THEN** viewport SHALL 显示：分隔线 + `› ` 提示符和输入框 + 分隔线 + 状态栏

#### Scenario: Slash 命令优先处理
- **WHEN** 用户输入以 `/` 开头的文本并按 Enter
- **THEN** 系统 SHALL 优先通过 `SlashCommand::from_input()` 识别并执行命令，不经过 @ 寻址解析

#### Scenario: 普通消息解析路由
- **WHEN** 用户输入非 `/` 开头的文本并按 Enter
- **THEN** 系统 SHALL 通过 `parse_input()` 解析寻址目标，然后通过校验函数校验 Agent 名称合法性

### Requirement: 退出方式
用户 SHALL 可以通过 `/quit`、`/exit` 命令或双击 Ctrl+C 退出程序。退出命令 SHALL 通过统一的 Slash 命令系统处理，不再硬编码。

#### Scenario: /quit 退出
- **WHEN** 用户在输入框中输入 `/quit` 并按 Enter
- **THEN** 程序 SHALL 通过 SlashCommand 系统识别并正常退出

#### Scenario: /exit 退出
- **WHEN** 用户在输入框中输入 `/exit` 并按 Enter
- **THEN** 程序 SHALL 正常退出，终端恢复原始状态
