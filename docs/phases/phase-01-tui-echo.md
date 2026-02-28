# Phase 1: 日志系统 + TUI Echo 模式

> 目标：搭建 TUI 框架和日志基础设施，用户输入什么就 echo 回显什么。

## 实现内容

- **日志系统**：基于 `tracing` + `tracing-subscriber`，日志写入 `.krew/logs/` 目录（文件日志，不输出到终端）
- **TUI 框架**：基于 `ratatui`，参考 codex CLI 的 TUI 实现（源码位于 `../codex`），搭建全屏终端界面
  - 上方：可滚动的输出区域
  - 下方：输入框，显示 `you>` 提示符
  - 启动时显示 ASCII banner（PDD §5.1 的 logo）
- **多行输入**：Shift+Enter 换行，Enter 发送（参考 codex 实现）
- **Echo 模式**：用户输入文本按 Enter 后，原样回显到输出区域（临时模式，后续替换为 LLM 调用）
- **退出**：支持 `/quit` 或 `Ctrl+C` 退出程序
- **CLI 参数**：`clap` 基础参数解析（`--verbose` 控制日志级别），已有骨架代码

## 验收标准

```txt
$ cargo run
┌─────────────────────────────────────┐
│  krew v0.1.0 — banner               │
│                                     │
│  you> hello world                   │
│  echo: hello world                  │
│                                     │
│  you> /quit                         │
│  Bye!                               │
└─────────────────────────────────────┘
```

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L293-297 | `.krew/logs/` 目录定义 |
| PDD | L421-437 | §5.1 启动界面 ASCII banner + Agent 列表 |
| PDD | L439-446 | §5.2 输入交互（Enter 发送、Shift+Enter 换行、Ctrl+C 中断） |
| PDD | L166-174 | §4.2.1 消息渲染格式（`you>` 前缀） |
| TDD | L93 | ratatui 选型 |
| TDD | L102-103 | tracing / tracing-subscriber 选型 |
| TDD | L670-677 | `.krew/` 存储路径结构 |
| TDD | L1007-1017 | krew-cli 源码结构（main.rs, app.rs, render.rs） |
| codex | `../codex` | TUI 实现参考（实现时研究具体结构） |
