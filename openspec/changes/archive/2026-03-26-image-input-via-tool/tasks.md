## 1. 数据类型定义

- [x] 1.1 在 `krew-tools/src/lib.rs` 中定义 `ImageContent` 结构体（`data: Vec<u8>`, `media_type: String`），新增 `MAX_IMAGE_SIZE: u64 = 20 * 1024 * 1024` 常量，并为 `ToolResult` 新增 `images: Vec<ImageContent>` 字段（默认空 Vec）
- [x] 1.2 在 `krew-llm/src/lib.rs` 中定义 `ImageContent` 结构体（`data: Vec<u8>`, `media_type: String`），并为 `ChatMessage` 新增 `images: Vec<ImageContent>` 字段（`#[serde(skip)]`，默认空 Vec）
- [x] 1.3 修复所有因 `ToolResult` 和 `ChatMessage` 新增字段导致的编译错误（补充 `..Default::default()` 或显式赋值 `images: vec![]`）

## 2. read_file 图片读取

- [x] 2.1 在 `krew-tools/src/builtin/read_file.rs` 中新增图片扩展名检测函数，支持 png/jpg/jpeg/gif/webp，返回对应 MIME type
- [x] 2.2 在 `ReadFileTool::execute()` 中，`check_binary()` 之前检查扩展名，若为图片则：检查文件大小是否超过 `MAX_IMAGE_SIZE`（20MB）、跳过 binary check、读取文件全部字节、构建 `ToolResult { content: "[Image: {filename}]", images: vec![ImageContent { data, media_type }], is_error: false }`
- [x] 2.3 更新 `ReadFileTool::spec()` 的 description，说明该工具支持读取图片文件（png/jpg/jpeg/gif/webp），LLM 可用此工具查看图片内容

## 3. agent_loop 图片传递

- [x] 3.1 在 `krew-core/src/agent/agent_loop.rs` 中，从 `ToolResult` 构建 `ChatMessage` 时，将 `ToolResult.images` 逐字段映射为 `ChatMessage.images`（`krew_tools::ImageContent` → `krew_llm::ImageContent`）

## 4. Provider 图片序列化

- [x] 4.1 `krew-llm/src/anthropic.rs`：修改 `convert_messages()` 中 `role: Tool` 分支，当 `images` 非空时将 `content` 从字符串改为数组格式，包含 `type: "image"` block（base64）和 `type: "text"` block
- [x] 4.2 `krew-llm/src/google.rs`：修改 `convert_messages()` 中 `role: Tool` 分支，当 `images` 非空时在 `functionResponse` **内部**新增 `parts` 数组包含 `inlineData`，同时添加 `id` 字段（从 `tool_call_id` 获取）和 `response.$ref` 引用；纯文本分支也应补上 `id` 字段
- [x] 4.3 `krew-llm/src/openai_responses.rs`：修改 `convert_messages()` 中 `role: Tool` 分支，当 `images` 非空时将 `output` 从字符串改为数组格式，包含 `type: "input_image"` block（data URI）和 `type: "text"` block
- [x] 4.4 `krew-llm/src/openai_chat.rs`：确认 `convert_messages()` 中 `role: Tool` 分支在 `images` 非空时仅使用文本 `content`（无需改动，但需确认行为正确）

## 5. 编译验证与测试

- [x] 5.1 运行 `cargo build` 确认全部编译通过
- [x] 5.2 运行 `cargo clippy --all-targets --all-features -- -D warnings` 确认无 lint 警告
- [x] 5.3 运行 `cargo test` 确认现有测试不被破坏
- [x] 5.4 为 `read_file` 图片读取功能添加集成测试（在 `crates/krew-tools/tests/` 下）
- [x] 5.5 为各 provider 的 `convert_messages()` 图片序列化添加单元测试
