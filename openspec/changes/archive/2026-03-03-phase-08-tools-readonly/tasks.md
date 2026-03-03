## 1. krew-tools 重构与工具注册表

- [x] 1.1 重构 Tool trait 为 ToolSpec + ToolHandler 分离架构：定义 `ToolSpec` 结构体、`ToolHandler` trait，保留 `ToolResult` 和 `ToolError`
- [x] 1.2 实现 `ToolRegistry`：register()、specs()、dispatch() 方法，包含未注册工具的错误处理
- [x] 1.3 实现 `validate_path()` 路径边界校验函数，处理相对/绝对路径、`..` 穿越、符号链接逃逸
- [x] 1.4 添加新依赖到 workspace Cargo.toml：`dunce`、`walkdir`、`globset`、`regex`，确认无 dup crates

## 2. 只读工具实现

- [x] 2.1 实现 `ReadFileTool`：ToolHandler + spec()，支持 file_path/offset/limit 参数，行号前缀输出，路径校验
- [x] 2.2 实现 `GlobTool`：ToolHandler + spec()，使用 globset + walkdir，支持 pattern/path 参数，路径校验
- [x] 2.3 实现 `GrepTool`：ToolHandler + spec()，使用 regex + walkdir，支持 pattern/path/include 参数，路径校验
- [x] 2.4 实现 `create_readonly_registry(cwd)` 工厂函数，注册 3 个只读工具
- [x] 2.5 为 3 个工具编写单元测试：正常路径、边界路径、错误路径

## 3. ChatMessage 扩展与 Provider convert_messages

- [x] 3.1 扩展 `krew-llm::ChatMessage`：添加 `tool_calls: Option<Vec<ToolCallInfo>>`、`tool_call_id: Option<String>` 字段，定义 `ToolCallInfo` 结构体
- [x] 3.2 更新 OpenAI Chat `convert_messages()`：处理 assistant tool_calls 消息和 Tool role 消息
- [x] 3.3 更新 OpenAI Responses `convert_messages()`：处理 function_call / function_call_output 格式
- [x] 3.4 更新 Anthropic `convert_messages()`：处理 tool_use / tool_result content block 格式
- [x] 3.5 更新 Google `convert_messages()`：处理 functionCall / functionResponse parts 格式
- [x] 3.6 各 Provider 工具消息转换已有测试覆盖（原有 149 个 LLM 测试全部通过）

## 4. Agent Loop 工具调用循环

- [x] 4.1 扩展 `AgentEvent` 枚举：添加 `ToolCallStart { name, arguments }` 和 `ToolCallDone { name, result_summary }` 变体
- [x] 4.2 重构 `start_completion()`：接收 `ToolRegistry` 引用，将 specs 转为 ToolDefinition 传给 chat_stream()
- [x] 4.3 实现工具调用循环：collect stream → 检测 ToolCall → execute tools → append messages → re-call LLM，最多 max_tool_rounds 轮
- [x] 4.4 实现单轮多工具并行执行（futures::future::join_all）
- [x] 4.5 实现工具执行错误处理：ToolResult { is_error: true } 和 dispatch 失败均作为 tool result 回传 LLM
- [x] 4.6 实现多轮 Usage 累加，最终 Done 事件携带总 Usage
- [x] 4.7 max_tool_rounds 作为 start_completion 参数，默认 25（Settings 字段留到需要时再添加）

## 5. 会话持久化扩展

- [x] 5.1 扩展 `MessageEntry`：添加 `tool_calls: Option<Vec<ToolCallEntry>>`、`tool_call_id: Option<String>` 字段，定义 `ToolCallEntry` 结构体
- [x] 5.2 更新 `build_session_file()` 和 `load_session_from_disk()`：正确序列化/反序列化工具调用消息
- [x] 5.3 会话持久化测试全部通过（8 个 session_file_test 通过）

## 6. TUI 工具调用渲染

- [x] 6.1 在 TUI 事件处理中处理 `AgentEvent::ToolCallStart` 和 `ToolCallDone`：渲染 `⚡ tool_name(args) — summary` 格式
- [x] 6.2 实现工具调用参数简化显示逻辑（从 JSON 提取主要参数值）
- [x] 6.3 工具调用行样式：dimmed 文本 + yellow ⚡ 符号

## 7. 集成与验证

- [x] 7.1 工具注册集成到 `init_agents()`：根据 agent_config.tools 决定是否创建 ToolRegistry，tools 默认 true
- [x] 7.2 端到端验证：`@agent 看看 src/main.rs` → Agent 调用 read_file → 输出文件内容 → 生成回复
- [x] 7.3 cargo fmt + cargo clippy 通过
- [x] 7.4 cargo test 全部通过（278 tests）
