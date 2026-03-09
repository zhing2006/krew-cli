# Phase 4: 单 LLM 接入（OpenAI Chat Completions）+ Markdown 渲染

> 目标：接入第一个 LLM Provider，实现流式对话和 Markdown 渲染，跑通完整的单 Agent 对话。

## 实现内容

- **OpenAI Chat Client**：实现 `LlmClient` trait，`POST /v1/chat/completions` (stream=true)
  - SSE 解析（`eventsource-stream`）
  - `StreamEvent` 映射（TextDelta、Done + Usage）
  - 错误处理与重试（429/5xx/超时/认证错误）
  - 采样参数映射（temperature、max_completion_tokens 等）
- **Agent Loop（单 Agent）**：
  - 构建 `ChatMessage` → 发送 LLM 请求 → 流式渲染回复
  - 暂不支持工具调用（跳过 ToolCall 事件）
- **流式渲染**：Agent 回复逐 token 实时渲染到 TUI 输出区
- **Markdown 渲染**：基于 `syntect` 的代码块语法高亮，列表/粗体/斜体等基础 Markdown 格式
- **Agent 标识**：回复带颜色标签 `[gpt] GPT-5.2:`

## 验收标准

```txt
you> @gpt 用 Rust 写一个 hello world

[gpt] GPT-5.2:
  以下是一个简单的 Rust hello world 程序：
  ```rust
  fn main() {
      println!("Hello, world!");
  }
  ```
```

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L74-84 | US-1 多模型对比问答（对话示例） |
| PDD | L166-178 | §4.2 消息渲染格式（Agent 标签 + 颜色 + 缩进） |
| PDD | L487-488 | §5.4 流式输出 |
| TDD | L228-269 | §3.3.1 LlmClient trait、StreamEvent、Usage |
| TDD | L283-288 | §3.3.2 OpenAI Chat Completions API 细节 |
| TDD | L359-373 | §3.3.4 错误处理与重试策略 |
| TDD | L387-418 | §3.3.6 采样参数映射表 |
| TDD | L897-937 | §5.1 消息发送流程（单 Agent） |
| TDD | L99 | syntect 代码语法高亮选型 |
