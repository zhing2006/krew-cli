# Phase 9: 工具系统 — 写入工具 + 审批流

> 目标：实现写操作工具和 Shell 工具，配套工具审批机制。

## 实现内容

- **写操作工具**：
  - `write_file` — 写入/创建文件
  - `edit_file` — 基于搜索替换的编辑
- **Shell 工具**：`shell` — 执行 Shell 命令
- **审批流程**：
  - `suggest` 模式：写操作 + Shell + MCP 需确认
  - `auto-edit` 模式：写操作自动，Shell + MCP 需确认
  - `full-auto` 模式：全部自动
  - 审批 UI：`⚡ shell("cargo test") — 允许? [y/n]`
- **路径边界**：写操作同样受 cwd 边界限制

## 验收标准

```txt
you> @opus 帮我在 src/ 下创建一个 utils.rs

[opus] Claude Opus:
  ⚡ write_file("src/utils.rs") — 允许? [y/n] y
  已创建 src/utils.rs，内容如下...
```

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L204-215 | §4.4.1 内置工具列表（读写分类） |
| PDD | L228-236 | §4.4.3 工具审批策略表 |
| PDD | L529-536 | §7.2 安全要求（路径边界、命令安全） |
| TDD | L449-458 | §3.4.2 内置工具列表（各审批策略下的行为） |
| TDD | L478-500 | §3.4.4 工具审批流程图 |
| TDD | L420-427 | §3.3.7 安全边界（路径校验、符号链接） |
