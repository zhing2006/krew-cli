## Context

krew-cli 的 LLM 消息链路为纯文本设计：`ToolResult { content: String }` → `ChatMessage { content: String }` → provider `convert_messages()` → JSON body。三家 API（Anthropic / Google / OpenAI Responses）均支持在 tool_result 中携带 base64 图片，OpenAI Chat Completions 不支持。

crate 依赖关系中 `krew-tools` 与 `krew-llm` 互不依赖，两者均被 `krew-core` 引用。因此图片数据类型需在两侧各自定义，由 `krew-core` 的 `agent_loop` 负责映射。

## Goals / Non-Goals

**Goals:**
- `read_file` 工具自动识别图片文件（png/jpg/jpeg/gif/webp），读取原始字节（`Vec<u8>`）并返回
- 图片数据通过 `ToolResult` → `ChatMessage` → provider 链路传递至 LLM API（base64 编码仅在 provider 序列化时执行）
- Anthropic / Google / OpenAI Responses 三个 provider 正确序列化图片内容
- OpenAI Chat Completions 降级处理，文本占位提示

**Non-Goals:**
- TUI 中展示图片内容（仅显示文本占位）
- 用户消息层面的图片自动检测（不在用户输入时解析路径）
- 图片输出/生成能力
- 音频、视频等其他多模态支持
- 图片数据的 session 持久化
- 跨 Agent 图片共享（当前架构下其他 Agent 的 tool chain 会被折叠为纯文本摘要，图片数据在折叠过程中丢失；仅调用 `read_file` 的 Agent 自身能看到图片）

## Decisions

### 1. 图片数据类型分别定义，`krew-core` 负责映射

`krew-tools` 和 `krew-llm` 互不依赖，不为此引入新的 crate 间依赖。

- `krew-tools`: `ToolResult` 新增 `images: Vec<ImageContent>` 字段
- `krew-llm`: `ChatMessage` 新增 `images: Vec<ImageContent>` 字段
- `krew-core`: `agent_loop.rs` 在从 `ToolResult` 构建 `ChatMessage` 时做字段映射

两侧的 `ImageContent` 结构相同：`data: Vec<u8>` + `media_type: String`，映射代码仅需逐字段复制。

**备选方案**：定义在共享 crate 或新建 `krew-types` crate → 过度设计，仅 2 个字段不值得。

### 2. 通过 `read_file` 工具的扩展名判断触发图片读取

在 `read_file` 执行时，在 `check_binary()` **之前**检查文件扩展名：

```
支持的扩展名: .png, .jpg, .jpeg, .gif, .webp
```

匹配到图片扩展名时：
1. 跳过 `check_binary()`（图片必然是 binary）
2. 检查文件大小是否超过图片专用上限（20MB）
3. 读取文件全部 bytes
4. 根据扩展名推断 MIME type
5. 返回 `ToolResult { content: "[Image: filename.png]", images: vec![...], is_error: false }`

`content` 字段保留文本描述，供日志和 TUI 展示。

**备选方案**：通过 magic bytes 判断文件类型 → 增加复杂度，扩展名判断已足够可靠。

### 3. 图片大小上限 20MB

工具层全局上限为 100MB（`MAX_FILE_SIZE`），但图片场景需要更低的专用上限。原因：

- Gemini API 总请求体限制 20MB（含 prompt + 所有 inline data）
- base64 编码后体积增长约 33%，20MB 原始文件编码后约 27MB
- 图片数据会常驻 messages 历史，后续每次 LLM 调用都会重复携带
- `prepare_messages_for_agent()` 对自身的 tool chain 保留原样，`prune_stale_tool_calls()` 仅在后续同文件操作后才清理旧结果

因此新增 `MAX_IMAGE_SIZE: u64 = 20 * 1024 * 1024`（20MB），在图片扩展名匹配后、读取文件前检查。

**备选方案**：沿用 100MB 上限交由 API 报错 → 内存浪费严重，且后续每轮请求都会携带超大 payload。

### 4. 各 provider 的图片序列化格式

#### Anthropic

tool_result 的 `content` 从字符串变为数组格式：

```json
{
  "type": "tool_result",
  "tool_use_id": "...",
  "content": [
    { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "..." } },
    { "type": "text", "text": "[Image: file.png]" }
  ]
}
```

#### Google Gemini

图片数据嵌入 `functionResponse` **内部**的 `parts` 字段，且必须携带与 `functionCall` 对应的 `id`。response 字段通过 `$ref` 引用图片的 `displayName`：

```json
{
  "role": "user",
  "parts": [
    {
      "functionResponse": {
        "name": "read_file",
        "id": "matching-function-call-id",
        "response": {
          "image_ref": { "$ref": "file.png" }
        },
        "parts": [
          {
            "inlineData": {
              "displayName": "file.png",
              "mimeType": "image/png",
              "data": "..."
            }
          }
        ]
      }
    }
  ]
}
```

**注意**：现有代码中 `convert_messages()` 的 `functionResponse` 未携带 `id` 字段，需一并修正。`id` 应从 `ChatMessage.tool_call_id` 获取。

#### OpenAI Responses

`output` 从字符串变为数组格式：

```json
{
  "type": "function_call_output",
  "call_id": "...",
  "output": [
    { "type": "input_image", "image_url": "data:image/png;base64,...", "detail": "auto" },
    { "type": "input_text", "text": "[Image: file.png]" }
  ]
}
```

**验证状态**：已验证到文档级别。OpenAI function-calling guide 明确声称 function output 可传 image/file 数组；API reference 将 `function_call_output.output` 定义为 `string | array`，数组成员包含 `ResponseInputTextContent`（`type: "input_text"`）、`ResponseInputImageContent`（`type: "input_image"`）、`ResponseInputFileContent`。`image_url` 支持完整 URL 和 base64 data URL。仍建议做一次真实 API 集成测试确认行为。

#### OpenAI Chat Completions（降级）

忽略 `images` 字段，`content` 保持为纯文本字符串，无需结构变更。

### 5. `read_file` 工具描述更新

更新 tool spec 的 `description`，增加图片读取能力的说明，使 LLM 知道可以用此工具查看图片文件。

### 6. 图片数据不持久化

`ChatMessage.images` 标记 `#[serde(skip)]`，序列化/反序列化时忽略。加载历史 session 时图片数据为空，不影响对话继续。

**备选方案**：存储文件路径并按需重新加载 → 增加复杂度，且路径可能失效，收益不大。

## Risks / Trade-offs

- **[图片常驻历史]** 图片数据在同一 Agent 的后续所有 LLM 调用中重复携带，直到被 `prune_stale_tool_calls` 清理 → 通过 20MB 上限控制单次读取大小，可接受
- **[Session 恢复]** 加载历史 session 后图片数据丢失，Agent 无法引用之前的图片 → 可接受，图片通常是即时查看用途
- **[扩展名误判]** 文件扩展名与实际内容不匹配时可能发送损坏数据 → 低概率，API 侧会返回错误，不做额外 magic bytes 校验
- **[OpenAI Chat 降级]** 使用 Chat Completions API 的用户无法使用图片功能 → 新模型已迁移到 Responses API，影响面小
- **[OpenAI Responses 需集成测试]** function_call_output 带图片数组已验证到文档级别（guide + API reference 均确认），但仍建议做一次真实 API 集成测试确认行为，失败则降级为纯文本
- **[跨 Agent 不可见]** 其他 Agent 看不到图片内容，只看到占位文本 → 作为 Non-Goal 接受，后续可通过改进 `prepare_messages_for_agent` 解决
