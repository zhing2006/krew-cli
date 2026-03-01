## ADDED Requirements

### Requirement: CommonMark 解析
Markdown 渲染器 SHALL 使用 `pulldown_cmark` 解析 CommonMark 格式文本，产出 `Vec<Line<'static>>` (ratatui Line 类型)。

#### Scenario: 纯文本
- **WHEN** 输入为 `"hello world"`
- **THEN** SHALL 渲染为一个无样式的 `Line`

#### Scenario: 多段落
- **WHEN** 输入包含由空行分隔的多个段落
- **THEN** SHALL 渲染为多个 `Line`，段落间有空行分隔

### Requirement: 行内样式
渲染器 SHALL 将 Markdown 行内标记映射为 ratatui Style。

#### Scenario: 粗体
- **WHEN** 输入包含 `**bold text**`
- **THEN** SHALL 渲染为 `Style::new().bold()` 样式的 Span

#### Scenario: 斜体
- **WHEN** 输入包含 `*italic text*`
- **THEN** SHALL 渲染为 `Style::new().italic()` 样式的 Span

#### Scenario: 行内代码
- **WHEN** 输入包含 `` `inline code` ``
- **THEN** SHALL 渲染为 `Style::new().cyan()` 样式的 Span

#### Scenario: 删除线
- **WHEN** 输入包含 `~~strikethrough~~`
- **THEN** SHALL 渲染为 `Style::new().crossed_out()` 样式的 Span

#### Scenario: 链接
- **WHEN** 输入包含 `[text](url)`
- **THEN** SHALL 渲染为 `Style::new().cyan().underlined()` 样式的 Span

#### Scenario: 样式嵌套
- **WHEN** 输入包含 `***bold italic***`
- **THEN** SHALL 渲染为同时带 bold 和 italic 的 Span

### Requirement: 标题样式
渲染器 SHALL 为不同级别的标题应用不同的样式。

#### Scenario: H1
- **WHEN** 输入包含 `# Heading 1`
- **THEN** SHALL 渲染为 `Style::new().bold().underlined()` 样式

#### Scenario: H2
- **WHEN** 输入包含 `## Heading 2`
- **THEN** SHALL 渲染为 `Style::new().bold()` 样式

#### Scenario: H3
- **WHEN** 输入包含 `### Heading 3`
- **THEN** SHALL 渲染为 `Style::new().bold().italic()` 样式

### Requirement: 列表渲染
渲染器 SHALL 正确渲染有序和无序列表，包含适当的缩进和标记。

#### Scenario: 无序列表
- **WHEN** 输入包含 `- item 1\n- item 2`
- **THEN** SHALL 渲染为带 `• ` 前缀的行，每项一行

#### Scenario: 有序列表
- **WHEN** 输入包含 `1. first\n2. second`
- **THEN** SHALL 渲染为带数字序号前缀的行

#### Scenario: 嵌套列表
- **WHEN** 输入包含嵌套列表
- **THEN** SHALL 渲染为带层级缩进的行

### Requirement: 引用块
渲染器 SHALL 渲染引用块，使用绿色样式和适当的前缀。

#### Scenario: 基本引用
- **WHEN** 输入包含 `> quoted text`
- **THEN** SHALL 渲染为 `Style::new().green()` 样式

### Requirement: 代码块语法高亮
渲染器 SHALL 使用 `syntect` 对围栏代码块进行语法高亮。

#### Scenario: 带语言标记的代码块
- **WHEN** 输入包含 ` ```rust\nfn main() {}\n``` `
- **THEN** SHALL 使用 syntect 按 Rust 语法高亮渲染，每行转为带颜色的 ratatui Spans

#### Scenario: 无语言标记的代码块
- **WHEN** 输入包含 ` ```\nsome code\n``` `
- **THEN** SHALL 以等宽样式渲染，不进行语法高亮

#### Scenario: 超大代码块 fallback
- **WHEN** 代码块内容超过 512KB 或 10000 行
- **THEN** SHALL fallback 为纯文本渲染，不调用 syntect

#### Scenario: 未知语言
- **WHEN** 代码块标记的语言在 syntect 语法集中不存在
- **THEN** SHALL fallback 为纯等宽样式渲染
