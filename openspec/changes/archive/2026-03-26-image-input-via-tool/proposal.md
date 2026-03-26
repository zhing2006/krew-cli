## Why

目前 krew-cli 的三家 LLM provider（Anthropic / OpenAI / Google）仅实现了纯文本消息，未支持任何多模态能力。用户无法让 Agent 查看图片内容（如截图、设计稿、图表等），限制了 Agent 的实用性。三家 API 均已原生支持图片输入，实现成本较低。

## What Changes

- `read_file` 工具新增图片文件自动识别与读取能力，检测到图片扩展名（png/jpg/jpeg/gif/webp）时自动读取原始字节（`Vec<u8>`）返回，而非报错 "binary file"；图片大小上限 20MB
- `ToolResult`（krew-tools）新增 `images` 字段，携带图片数据（`Vec<ImageContent>`）
- `ChatMessage`（krew-llm）新增 `images` 字段，将图片数据从 tool result 传递到 LLM provider 层；两个 crate 各自定义 `ImageContent`，由 `krew-core` 负责映射
- Anthropic / Google / OpenAI Responses 三个 provider 的 `convert_messages()` 支持将图片数据序列化为各自 API 要求的格式
- OpenAI Chat Completions provider 降级处理：忽略图片，返回文本占位提示
- 图片数据不持久化到 session 文件（`#[serde(skip)]`）

## Capabilities

### New Capabilities
- `image-input`: 通过 `read_file` 工具读取图片文件并作为多模态内容发送给 LLM，覆盖 `ImageContent` 类型定义、图片格式检测、图片大小限制、以及 `ToolResult`/`ChatMessage` 的图片数据传递（`Vec<u8>` 原始字节在结构体间传递，base64 编码仅在 provider 序列化时执行）

### Modified Capabilities
- `message-types`: `ChatMessage` 新增 `images` 字段（`#[serde(skip)]`），用于携带图片数据
- `builtin-tools-readonly`: `read_file` 工具新增图片文件检测与读取逻辑，遇到支持的图片格式时跳过 binary check 并返回图片数据
- `anthropic-client`: `convert_messages()` 处理 tool_result 中的图片，序列化为 `type: "image"` content block
- `google-client`: `convert_messages()` 处理 tool_result 中的图片，序列化为 `inlineData` part
- `openai-responses-client`: `convert_messages()` 处理 tool_result 中的图片，序列化为 `type: "input_image"` content block
- `openai-chat-client`: `convert_messages()` 降级处理图片，输出文本占位提示

## Impact

- **Crate 改动**: `krew-llm`（`ImageContent` 类型 + 4 个 provider）、`krew-tools`（`ImageContent` 类型 + `ToolResult` + `read_file`）、`krew-core`（`agent_loop` 映射 images）
- **依赖方向**: `krew-tools` 与 `krew-llm` 互不依赖，`ImageContent` 在两侧各自定义（结构相同），由 `krew-core` 负责逐字段映射
- **Session 持久化**: 图片数据标记为 `#[serde(skip)]`，不影响现有 session 存储格式
- **用户体验**: 用户在消息中提及图片路径 → Agent 自主调用 `read_file` → Agent 能"看到"图片内容
- **无 BREAKING 变更**: 所有改动均为新增字段（带默认值），不影响现有功能
