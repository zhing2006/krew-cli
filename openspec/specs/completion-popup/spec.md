## ADDED Requirements

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

### Requirement: Agent 名称补全
输入框中光标位置存在 `@` token 时，SHALL 自动显示补全弹窗，列出匹配的 Agent 名称（包括 `all`）。

#### Scenario: 输入 @ 触发补全
- **WHEN** 用户输入 `@`
- **THEN** 弹窗 SHALL 显示 `all` 和所有配置的 Agent 名称

#### Scenario: 输入过滤
- **WHEN** 用户输入 `@gp`
- **THEN** 弹窗 SHALL 只显示前缀匹配的 Agent（如 `gpt`）

#### Scenario: 无匹配
- **WHEN** 用户输入 `@xyz`（无匹配 Agent）
- **THEN** 弹窗 SHALL 关闭

### Requirement: 弹窗布局
弹窗 SHALL 替换 viewport 底部的状态栏区域并向下扩展。viewport 高度 SHALL 动态增加以容纳弹窗。

#### Scenario: 弹窗扩展 viewport
- **WHEN** 弹窗激活显示 N 行内容
- **THEN** viewport 高度 SHALL 增加约 N 行，上方内容 SHALL 被推上（scroll up），弹窗显示在原状态栏位置

#### Scenario: 弹窗关闭恢复
- **WHEN** 弹窗关闭
- **THEN** viewport 高度 SHALL 恢复原始大小，状态栏 SHALL 重新显示

### Requirement: 弹窗键盘交互
弹窗激活时 SHALL 拦截导航按键。

#### Scenario: 上下箭头导航
- **WHEN** 弹窗激活时用户按下 ↑ 或 ↓
- **THEN** 选中项 SHALL 上移或下移（到边界时回绕）

#### Scenario: Tab 确认选择
- **WHEN** 弹窗激活时用户按下 Tab
- **THEN** 选中项 SHALL 插入到输入框（替换当前 token），弹窗关闭

#### Scenario: Enter 确认选择
- **WHEN** 弹窗激活且有选中项时用户按下 Enter
- **THEN** 对于 Slash 命令，SHALL 直接执行该命令；对于 Agent 名称，SHALL 插入到输入框

#### Scenario: Esc 关闭
- **WHEN** 弹窗激活时用户按下 Esc
- **THEN** 弹窗 SHALL 关闭，输入内容保持不变

#### Scenario: 继续输入更新过滤
- **WHEN** 弹窗激活时用户继续输入字符
- **THEN** 弹窗 SHALL 根据最新输入更新过滤结果

### Requirement: 互斥弹窗
同一时间 SHALL 只能显示一个弹窗。Slash 命令补全和 Agent 名称补全不能同时出现。

#### Scenario: 只有一个弹窗
- **WHEN** 输入同时满足 `/` 和 `@` 触发条件
- **THEN** 系统 SHALL 只显示优先级更高的弹窗（Slash 命令优先）

### Requirement: 弹窗选中项高亮
弹窗 SHALL 使用视觉样式区分选中项和非选中项。

#### Scenario: 选中项样式
- **WHEN** 弹窗显示且有选中项
- **THEN** 选中项 SHALL 以青色（Cyan）加粗显示，非选中项以默认样式显示
