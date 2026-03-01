## 1. 依赖与基础设施

- [x] 1.1 在 workspace Cargo.toml 中添加 `pulldown-cmark`、`syntect`、`two-face` 依赖，在 krew-llm 和 krew-cli 的 Cargo.toml 中引用
- [x] 1.2 在 krew-core 中定义 `AgentEvent` 枚举（ResponseStart、TextDelta、Done、Error），导出为公共 API

## 2. OpenAI Chat Client (krew-llm)

- [x] 2.1 实现 `OpenAiChatClient` 结构体，包含 `reqwest::Client`、base_url、api_key、model 等字段，并实现构造函数
- [x] 2.2 实现 `convert_messages()` — 将 `&[ChatMessage]` 转换为 OpenAI messages JSON 格式（含 self_agent_name 和 other_agent_role 参数）
- [x] 2.3 实现采样参数映射 — 将 `SamplingConfig` 转换为 OpenAI 请求 body 字段（temperature、max_completion_tokens、top_p、frequency_penalty、presence_penalty、stop）
- [x] 2.4 实现 `LlmClient::chat_stream()` — 构建 HTTP 请求、SSE 解析（eventsource-stream）、StreamEvent 映射（TextDelta/ToolCall/Done+Usage）、处理 `data: [DONE]`
- [x] 2.5 实现错误处理与重试逻辑 — 429 指数退避（1s→2s→4s，3次）、5xx 重试（2次，2s间隔）、首 token 超时 60s、401/403 不重试、流式中断返回 Error
- [x] 2.6 为 OpenAiChatClient 编写单元测试 — 消息格式转换、采样参数映射、SSE 事件解析（使用 mock response）

## 3. Agent Loop (krew-core)

- [x] 3.1 为 `AgentRuntime` 实现 `complete()` 方法 — 接收消息历史，spawn tokio task，通过 mpsc::unbounded_channel 发送 AgentEvent
- [x] 3.2 在 `complete()` 中实现 system prompt 注入 — 调用 `build_system_prompt()` 构建系统消息，插入消息列表头部
- [x] 3.3 在 `complete()` 中实现流式事件消费 — 从 `chat_stream()` 读取 StreamEvent，映射为 AgentEvent 发送（跳过 ToolCall）
- [x] 3.4 为 agent loop 编写单元测试 — 验证 AgentEvent 序列（ResponseStart → TextDelta* → Done）、错误传播、ToolCall 跳过

## 4. Markdown 渲染引擎 (krew-cli)

- [x] 4.1 创建 `render/markdown.rs` 模块 — 实现 `render_markdown(text: &str) -> Vec<Line<'static>>`，使用 pulldown_cmark 解析并映射行内样式（bold、italic、code、strikethrough、link）
- [x] 4.2 实现标题和引用块样式 — H1(bold+underlined)、H2(bold)、H3(bold+italic)、blockquote(green)
- [x] 4.3 实现列表渲染 — 无序列表（`• ` 前缀）、有序列表（数字前缀）、嵌套列表缩进
- [x] 4.4 创建 `render/highlight.rs` 模块 — 使用 syntect + two-face 实现 `highlight_code_to_lines(code: &str, lang: Option<&str>) -> Vec<Line<'static>>`，含超大代码块 fallback 和未知语言 fallback
- [x] 4.5 在 `render_markdown()` 中集成代码块高亮 — 遇到围栏代码块时调用 `highlight_code_to_lines()`
- [x] 4.6 为 Markdown 渲染编写单元测试 — 行内样式、标题、列表、代码块高亮、超大代码块 fallback

## 5. 流式渲染管线 (krew-cli)

- [x] 5.1 创建 `streaming/mod.rs` — 实现 `StreamState`（VecDeque<QueuedLine>，带时间戳入队、step、drain_n、drain_all、oldest_queued_age）
- [x] 5.2 创建 `streaming/chunking.rs` — 实现 `AdaptiveChunkingPolicy` 双模式状态机（Smooth/CatchUp，含迟滞阈值、重新进入冷却、严重积压逃逸）
- [x] 5.3 创建 `streaming/markdown_stream.rs` — 实现 `MarkdownStreamCollector`（push_delta、commit_complete_lines、finalize），内部调用 render_markdown
- [x] 5.4 创建 `streaming/commit_tick.rs` — 实现 commit tick 编排逻辑（snapshot → decide → drain → 返回待插入行）
- [x] 5.5 为流式管线编写单元测试 — StreamState FIFO 行为、AdaptiveChunkingPolicy 模式切换、MarkdownStreamCollector 增量行返回

## 6. Agent 显示 (krew-cli)

- [x] 6.1 实现 Agent 标签渲染 — 收到 ResponseStart 时插入彩色 `[name] DisplayName:` 行
- [x] 6.2 实现回复内容缩进 — 流式行插入时添加 2 空格前缀
- [x] 6.3 实现 Token 用量显示 — 收到 Done 时插入右对齐灰色 `── tokens: X in / Y out` 行（含千位分隔符）
- [x] 6.4 实现错误显示 — 收到 Error 时插入红色 `  ✗ {message}` 行

## 7. Agent 初始化与集成

- [x] 7.1 在 `App::new()` 中实现 agent 初始化 — 遍历 Config.agents，根据 provider 类型创建 OpenAiChatClient + AgentRuntime，存入 `HashMap<String, AgentRuntime>`
- [x] 7.2 修改 `send_message()` — 解析寻址后，若目标 agent 有 LlmClient 则调用 `agent.complete(messages)`；builtin agent 保持 echo 行为
- [x] 7.3 在事件循环中集成 AgentEvent 接收 — `select!` 中添加 `agent_event_rx.recv()` 分支，分发到流式管线处理
- [x] 7.4 实现消息历史管理 — 用户消息和 Agent 回复追加到 `Vec<ChatMessage>`，传递给 agent.complete()
- [x] 7.5 集成 commit tick 动画 — 首个 TextDelta 启动 FrameScheduler 持续 tick，Done + 队列空后停止
- [x] 7.6 端到端手动测试 — 配置 OpenAI agent，验证完整对话流程：用户输入 → Agent 标签 → 流式 Markdown 渲染 → Token 用量显示
