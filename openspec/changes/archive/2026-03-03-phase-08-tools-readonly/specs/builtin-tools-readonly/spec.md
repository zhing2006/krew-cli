## ADDED Requirements

### Requirement: read_file tool
`krew-tools` SHALL 提供 `ReadFileTool` 实现 `ToolHandler` trait，读取指定文件内容并返回。参数：`file_path: String`（必填）、`offset: Option<u32>`（起始行号，1-based）、`limit: Option<u32>`（读取行数）。返回内容 SHALL 包含行号前缀（格式：`{line_number} | {content}`）。

#### Scenario: 读取完整文件
- **WHEN** 调用 `read_file` 传入 `{ "file_path": "src/main.rs" }`（无 offset/limit）
- **THEN** SHALL 返回文件全部内容，每行带行号前缀

#### Scenario: 读取指定行范围
- **WHEN** 调用 `read_file` 传入 `{ "file_path": "src/main.rs", "offset": 10, "limit": 5 }`
- **THEN** SHALL 返回第 10-14 行的内容，行号从 10 开始

#### Scenario: 文件不存在
- **WHEN** 调用 `read_file` 传入不存在的文件路径
- **THEN** SHALL 返回 `ToolResult { is_error: true }` 并包含错误信息

#### Scenario: 路径越界
- **WHEN** 调用 `read_file` 传入越过 cwd 边界的路径（如 `../../etc/passwd`）
- **THEN** SHALL 返回 `ToolResult { is_error: true }` 提示路径越界

### Requirement: glob tool
`krew-tools` SHALL 提供 `GlobTool` 实现 `ToolHandler` trait，使用 `globset` + `walkdir` 进行文件名模式匹配。参数：`pattern: String`（必填，glob 模式如 `**/*.rs`）、`path: Option<String>`（搜索根目录，默认 cwd）。返回匹配的文件路径列表，每行一个相对路径。

#### Scenario: 匹配 Rust 源文件
- **WHEN** 调用 `glob` 传入 `{ "pattern": "**/*.rs" }`
- **THEN** SHALL 返回 cwd 下所有 `.rs` 文件的相对路径，每行一个

#### Scenario: 指定子目录
- **WHEN** 调用 `glob` 传入 `{ "pattern": "*.toml", "path": "crates/krew-cli" }`
- **THEN** SHALL 仅在 `crates/krew-cli` 目录下搜索

#### Scenario: 无匹配
- **WHEN** 调用 `glob` 传入不匹配任何文件的模式
- **THEN** SHALL 返回 `ToolResult { content: "No matches found", is_error: false }`

#### Scenario: 路径越界
- **WHEN** 调用 `glob` 传入越过 cwd 边界的 `path`
- **THEN** SHALL 返回 `ToolResult { is_error: true }` 提示路径越界

### Requirement: grep tool
`krew-tools` SHALL 提供 `GrepTool` 实现 `ToolHandler` trait，使用 ripgrep 底层 crate（`grep-searcher` + `grep-regex`）进行文件内容正则搜索。参数：`pattern: String`（必填，正则表达式）、`path: Option<String>`（搜索路径，默认 cwd）、`include: Option<String>`（glob 过滤，如 `*.rs`）。返回匹配结果，格式为 `{file_path}:{line_number}: {content}`。

#### Scenario: 基本搜索
- **WHEN** 调用 `grep` 传入 `{ "pattern": "TODO" }`
- **THEN** SHALL 返回 cwd 下所有包含 "TODO" 的行，格式为 `{file}:{line}: {content}`

#### Scenario: 带文件类型过滤
- **WHEN** 调用 `grep` 传入 `{ "pattern": "fn main", "include": "*.rs" }`
- **THEN** SHALL 仅在 `.rs` 文件中搜索

#### Scenario: 无匹配
- **WHEN** 搜索结果为空
- **THEN** SHALL 返回 `ToolResult { content: "No matches found", is_error: false }`

#### Scenario: 无效正则
- **WHEN** 传入无效的正则表达式
- **THEN** SHALL 返回 `ToolResult { is_error: true }` 并包含正则错误信息

#### Scenario: 路径越界
- **WHEN** 调用 `grep` 传入越过 cwd 边界的 `path`
- **THEN** SHALL 返回 `ToolResult { is_error: true }` 提示路径越界

### Requirement: 路径边界校验函数
`krew-tools` SHALL 提供公共函数 `validate_path(path: &str, cwd: &Path) -> Result<PathBuf, ToolError>`，解析相对路径为绝对路径并校验在 cwd 范围内。拒绝 `..` 穿越和符号链接逃逸。

#### Scenario: 合法相对路径
- **WHEN** 调用 `validate_path("src/main.rs", cwd)`
- **THEN** SHALL 返回 `Ok(cwd.join("src/main.rs")` 的 canonicalize 结果

#### Scenario: 穿越路径
- **WHEN** 调用 `validate_path("../../etc/passwd", cwd)`
- **THEN** SHALL 返回 `Err(ToolError::Execution(...))`

#### Scenario: 绝对路径在 cwd 内
- **WHEN** 调用 `validate_path("/project/src/main.rs", cwd)` 且 cwd 为 `/project`
- **THEN** SHALL 返回 `Ok` 因为路径在 cwd 范围内

#### Scenario: 绝对路径在 cwd 外
- **WHEN** 调用 `validate_path("/other/file.txt", cwd)` 且 cwd 为 `/project`
- **THEN** SHALL 返回 `Err(ToolError::Execution(...))`

### Requirement: ToolSpec 定义
每个内置工具 SHALL 提供 `fn spec() -> ToolSpec` 方法，返回包含 name、description、parameters（JSON Schema）的 `ToolSpec`。parameters SHALL 符合 JSON Schema draft-07 规范。

#### Scenario: read_file spec
- **WHEN** 获取 read_file 的 ToolSpec
- **THEN** parameters SHALL 包含 `file_path`（required string）、`offset`（optional integer）、`limit`（optional integer）

#### Scenario: glob spec
- **WHEN** 获取 glob 的 ToolSpec
- **THEN** parameters SHALL 包含 `pattern`（required string）、`path`（optional string）

#### Scenario: grep spec
- **WHEN** 获取 grep 的 ToolSpec
- **THEN** parameters SHALL 包含 `pattern`（required string）、`path`（optional string）、`include`（optional string）
