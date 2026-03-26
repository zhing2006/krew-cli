## ADDED Requirements

### Requirement: ImageContent 数据类型（krew-tools）
`krew-tools` SHALL 定义 `ImageContent` 结构体，包含字段：`data: Vec<u8>`（图片原始字节）、`media_type: String`（MIME type，如 `"image/png"`）。该类型 SHALL 实现 `Debug` 和 `Clone`。

#### Scenario: ImageContent 结构体可构造
- **WHEN** 构造 `ImageContent { data: vec![...], media_type: "image/png".to_string() }`
- **THEN** SHALL 成功创建实例，所有字段可访问

### Requirement: ImageContent 数据类型（krew-llm）
`krew-llm` SHALL 定义 `ImageContent` 结构体，包含字段：`data: Vec<u8>`（图片原始字节）、`media_type: String`（MIME type）。该类型 SHALL 实现 `Debug`、`Clone`，且 `#[serde(skip)]` 标记使其在序列化/反序列化时被忽略。

#### Scenario: ImageContent 结构体可构造
- **WHEN** 构造 `krew_llm::ImageContent { data: vec![...], media_type: "image/jpeg".to_string() }`
- **THEN** SHALL 成功创建实例，所有字段可访问

#### Scenario: 序列化时被忽略
- **WHEN** 对包含 `images` 字段的 `ChatMessage` 进行 `serde_json::to_string()`
- **THEN** 输出 JSON SHALL 不包含 `images` 键

### Requirement: 支持的图片格式
系统 SHALL 支持以下图片格式（按扩展名判断）：`.png`（`image/png`）、`.jpg`（`image/jpeg`）、`.jpeg`（`image/jpeg`）、`.gif`（`image/gif`）、`.webp`（`image/webp`）。

#### Scenario: PNG 文件识别
- **WHEN** `read_file` 接收到 `file_path` 扩展名为 `.png` 的文件
- **THEN** SHALL 识别为图片文件，`media_type` 为 `"image/png"`

#### Scenario: JPG 文件识别
- **WHEN** `read_file` 接收到 `file_path` 扩展名为 `.jpg` 的文件
- **THEN** SHALL 识别为图片文件，`media_type` 为 `"image/jpeg"`

#### Scenario: JPEG 文件识别
- **WHEN** `read_file` 接收到 `file_path` 扩展名为 `.jpeg` 的文件
- **THEN** SHALL 识别为图片文件，`media_type` 为 `"image/jpeg"`

#### Scenario: GIF 文件识别
- **WHEN** `read_file` 接收到 `file_path` 扩展名为 `.gif` 的文件
- **THEN** SHALL 识别为图片文件，`media_type` 为 `"image/gif"`

#### Scenario: WebP 文件识别
- **WHEN** `read_file` 接收到 `file_path` 扩展名为 `.webp` 的文件
- **THEN** SHALL 识别为图片文件，`media_type` 为 `"image/webp"`

#### Scenario: 不支持的扩展名
- **WHEN** `read_file` 接收到 `.bmp`、`.tiff`、`.svg` 等不在支持列表中的扩展名
- **THEN** SHALL 按照原有逻辑处理（作为文本或 binary 检测）

### Requirement: 图片文件大小限制
`read_file` 对图片文件 SHALL 使用专用大小上限 20MB（`MAX_IMAGE_SIZE`），而非通用的 100MB（`MAX_FILE_SIZE`）。

#### Scenario: 图片文件在 20MB 以内
- **WHEN** 调用 `read_file` 传入一个 15MB 的 PNG 文件
- **THEN** SHALL 正常读取并返回图片数据

#### Scenario: 图片文件超过 20MB
- **WHEN** 调用 `read_file` 传入一个 25MB 的 PNG 文件
- **THEN** SHALL 返回 `ToolResult { is_error: true, .. }` 提示图片文件过大

#### Scenario: 非图片文件仍使用 100MB 上限
- **WHEN** 调用 `read_file` 传入一个 50MB 的文本文件
- **THEN** SHALL 使用原有 `MAX_FILE_SIZE`（100MB）上限检查

### Requirement: agent_loop 图片数据映射
`krew-core` 的 agent_loop SHALL 在从 `ToolResult` 构建 `ChatMessage`（`role: Tool`）时，将 `ToolResult.images` 逐字段映射为 `ChatMessage.images`（`krew_tools::ImageContent` → `krew_llm::ImageContent`）。

#### Scenario: 有图片的 tool result 映射
- **WHEN** `ToolResult` 包含非空 `images` 字段
- **THEN** 构建的 `ChatMessage` 的 `images` 字段 SHALL 包含相同数量和内容的 `ImageContent`

#### Scenario: 无图片的 tool result 映射
- **WHEN** `ToolResult` 的 `images` 为空
- **THEN** 构建的 `ChatMessage` 的 `images` 字段 SHALL 为空 `Vec`
