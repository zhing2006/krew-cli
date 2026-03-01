## ADDED Requirements

### Requirement: 启动 banner 显示 Agent 列表
`krew-cli` 的启动 banner SHALL 包含一行 Agents 信息，格式为：
```
Agents: [name] DisplayName | [name] DisplayName | ...
```
Agent 列表 SHALL 按 `config.settings.reply_order` 的顺序排列。每个 `[name]` SHALL 使用该 agent 配置的 `color` 着色。

#### Scenario: 显示多个 agent
- **WHEN** 配置中有 3 个 agent（gpt/green, opus/magenta, gemini/blue）
- **THEN** banner 中 SHALL 显示 `Agents: [gpt] GPT-5.2 | [opus] Claude Opus | [gemini] Gemini 3.1 Pro`
- **AND** 各 `[name]` 部分使用各自的 color

#### Scenario: 默认 echo 模式
- **WHEN** 使用默认配置（无 settings.toml）
- **THEN** banner 中 SHALL 显示 `Agents: [echo] Echo`

### Requirement: 状态栏显示实际配置值
`krew-cli` 的状态栏 SHALL 从配置中读取并显示：
- `approval_mode` 的当前值（如 `suggest`、`auto-edit`、`full-auto`）
- `auto_compact_threshold` 的值（如 `120k`）或 `off`（当为 `None` 或 `0` 时）

#### Scenario: 状态栏反映配置
- **WHEN** 配置中 `approval_mode` 为 `FullAuto`，`auto_compact_threshold` 为 `Some(120000)`
- **THEN** 状态栏 SHALL 显示 `full-auto` 和 `120k`

#### Scenario: auto-compact 禁用
- **WHEN** `auto_compact_threshold` 为 `None`
- **THEN** 状态栏 SHALL 显示 `auto-compact off`

### Requirement: Banner 布局为三行内容
启动 banner SHALL 包含 3 行内容（加边框共 5 行）：第一行为标题和版本，第二行为 Agent 列表，第三行左对齐 `Directory:` + 路径、右对齐 `Type /help for commands`。路径超过可用宽度时 SHALL 使用中间截断（`...`）。

#### Scenario: 路径截断
- **WHEN** 路径长度超过 banner 可用宽度
- **THEN** SHALL 截断为 `H:\ZHing\...ew-cli` 形式，保留首尾部分

### Requirement: Agent 颜色字符串映射
`krew-cli` SHALL 支持将 agent 配置中的 `color` 字符串映射到 ratatui `Color` 枚举值。SHALL 支持的颜色名称至少包括：`red`、`green`、`yellow`、`blue`、`magenta`、`cyan`、`white`。无法识别的颜色 SHALL 回退到 `Color::White`。

#### Scenario: 已知颜色映射
- **WHEN** agent 的 `color` 为 `"magenta"`
- **THEN** SHALL 映射为 `Color::Magenta`

#### Scenario: 未知颜色回退
- **WHEN** agent 的 `color` 为 `"rainbow"`
- **THEN** SHALL 回退为 `Color::White`

### Requirement: 消息前缀样式
用户消息 SHALL 使用 `> ` 前缀（绿色粗体），agent 回复 SHALL 使用 `● ` 前缀（使用 agent 配置的颜色，粗体）。`insert_message()` SHALL 接收 agent 颜色参数。多行消息续行 SHALL 按前缀的 Unicode 显示宽度对齐。

#### Scenario: 用户消息
- **WHEN** 用户发送消息
- **THEN** SHALL 显示为 `> 消息内容`（绿色）

#### Scenario: Agent 回复颜色跟随配置
- **WHEN** agent 配置颜色为 `"magenta"`
- **THEN** 回复前缀 `● ` SHALL 使用 magenta 颜色

### Requirement: CJK 宽字符渲染
`custom_terminal::draw_cells()` SHALL 使用 `unicode-width` 计算 cell 显示宽度，跳过宽字符（CJK、emoji）的续接 cell，避免覆盖宽字符右半部分导致中文间出现空格。

#### Scenario: 中文消息无多余空格
- **WHEN** 用户输入中文文本如 `你好世界`
- **THEN** 显示时各字符之间 SHALL 无多余空格
