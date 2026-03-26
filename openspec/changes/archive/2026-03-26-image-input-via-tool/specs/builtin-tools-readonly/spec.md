## MODIFIED Requirements

### Requirement: read_file tool
`krew-tools` SHALL 提供 `ReadFileTool` 实现 `ToolHandler` trait，读取指定文件内容并返回。参数：`file_path: String`（必填）、`offset: Option<u32>`（起始行号，1-based）、`limit: Option<u32>`（读取行数）。对于文本文件，返回内容 SHALL 包含行号前缀（格式：`{line_number} | {content}`）。对于支持的图片文件（png/jpg/jpeg/gif/webp），SHALL 读取文件字节并通过 `ToolResult.images` 返回。

#### Scenario: 读取完整文件
- **WHEN** 调用 `read_file` 传入 `{ "file_path": "src/main.rs" }`（无 offset/limit）
- **THEN** SHALL 返回文件全部内容，每行带行号前缀

#### Scenario: 读取指定行范围
- **WHEN** 调用 `read_file` 传入 `{ "file_path": "src/main.rs", "offset": 10, "limit": 5 }`
- **THEN** SHALL 返回第 10-14 行的内容，行号从 10 开始

#### Scenario: 文件不存在
- **WHEN** 调用 `read_file` 传入不存在的文件路径
- **THEN** SHALL 返回 `ToolResult { is_error: true, .. }` 并包含错误信息

#### Scenario: 路径越界
- **WHEN** 调用 `read_file` 传入越过 cwd 边界的路径（如 `../../etc/passwd`）
- **THEN** SHALL 返回 `ToolResult { is_error: true, .. }` 提示路径越界

#### Scenario: 读取 PNG 图片
- **WHEN** 调用 `read_file` 传入 `{ "file_path": "screenshot.png" }`
- **THEN** SHALL 跳过 binary 检测，读取文件字节，返回 `ToolResult { content: "[Image: screenshot.png]", images: vec![ImageContent { data, media_type: "image/png" }], is_error: false }`

#### Scenario: 读取 JPEG 图片
- **WHEN** 调用 `read_file` 传入 `{ "file_path": "photo.jpg" }`
- **THEN** SHALL 返回 `ToolResult { content: "[Image: photo.jpg]", images: vec![ImageContent { data, media_type: "image/jpeg" }], is_error: false }`

#### Scenario: 图片文件不存在
- **WHEN** 调用 `read_file` 传入不存在的图片路径 `{ "file_path": "missing.png" }`
- **THEN** SHALL 返回 `ToolResult { is_error: true, .. }` 并包含错误信息

#### Scenario: 图片文件超过大小限制
- **WHEN** 调用 `read_file` 传入超过 `MAX_IMAGE_SIZE`（20MB）的图片文件
- **THEN** SHALL 返回 `ToolResult { is_error: true, .. }` 提示图片文件过大

#### Scenario: 图片读取忽略 offset 和 limit
- **WHEN** 调用 `read_file` 传入 `{ "file_path": "img.png", "offset": 10, "limit": 5 }`
- **THEN** SHALL 忽略 offset 和 limit 参数，读取完整图片文件

### Requirement: ToolSpec 定义
每个内置工具 SHALL 提供 `fn spec() -> ToolSpec` 方法，返回包含 name、description、parameters（JSON Schema）的 `ToolSpec`。parameters SHALL 符合 JSON Schema draft-07 规范。

#### Scenario: read_file spec
- **WHEN** 获取 read_file 的 ToolSpec
- **THEN** parameters SHALL 包含 `file_path`（required string）、`offset`（optional integer）、`limit`（optional integer）

#### Scenario: read_file description 包含图片能力说明
- **WHEN** 获取 read_file 的 ToolSpec
- **THEN** description SHALL 说明该工具可以读取图片文件（png/jpg/jpeg/gif/webp），LLM 可用此工具查看图片内容
