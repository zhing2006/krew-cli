# Phase 8: 工具系统 — 只读工具

> 目标：实现只读工具（read_file、glob、grep），Agent 可以读取项目文件。

## 实现内容

- **Tool trait 实现**：已有骨架，实现具体工具逻辑
- **只读工具**：
  - `read_file` — 读取文件内容（支持行号范围）
  - `glob` — 文件名模式匹配
  - `grep` — 文件内容搜索（正则）
- **Agent Loop 扩展**：处理 `StreamEvent::ToolCall`，执行工具，将结果回传 LLM
- **工具输出渲染**：`⚡ read_file("src/main.rs")` 格式显示
- **路径边界**：文件路径必须在 `session.cwd` 内，拒绝 `..` 穿越
- **审批策略**：只读工具在所有策略下自动执行（无需确认）

## 验收标准

```txt
you> @opus 看看 src/main.rs 有什么问题

[opus] Claude Opus:
  ⚡ read_file("src/main.rs")
  我看了你的代码，第 15 行有一个...
```

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L125-135 | US-5 工具协作 |
| PDD | L204-215 | §4.4.1 内置工具列表 |
| PDD | L180-186 | §4.2.3 工具调用显示格式 |
| PDD | L466-474 | §5.3 工具调用输出渲染 |
| TDD | L429-458 | §3.4 工具系统（Tool trait、ToolResult、工具表） |
| TDD | L420-427 | §3.3.7 路径边界安全 |
| TDD | L968-1001 | §5.3 工具调用流程 |
| TDD | L1039-1051 | krew-tools 源码结构 |
