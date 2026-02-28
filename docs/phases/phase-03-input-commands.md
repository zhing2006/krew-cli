# Phase 3: 输入解析 + Slash 命令

> 目标：解析 `@` 寻址和 `/` 命令，Slash 命令本地执行。

## 实现内容

- **@ 寻址解析**：`parse_input()` 已有骨架，补充对已加载 Agent 的校验（未知 Agent 报错）
- **Slash 命令执行**：
  - `/help` — 显示命令列表
  - `/agents` — 列出当前 Agent 及状态（暂无 token 统计）
  - `/clear` — 清屏
  - `/quit` — 退出
  - `/new`、`/resume`、`/compact` — 暂时提示"功能待实现"
- **命令补全**：输入 `/` 后提示可用命令列表
- **输入历史**：上下箭头浏览历史输入
- **Echo 升级**：echo 回显时显示解析结果（`[→ @all]` / `[→ @gpt]` / `[→ last]`）

## 验收标准

```txt
you> @all hello
[→ @all] echo: hello

you> @gpt explain this
[→ @gpt] echo: explain this

you> /help
Available commands: /new, /resume, /agents, /clear, /compact, /help, /quit

you> /agents
Agents in session:
  [gpt]  GPT-5.2      openai/gpt-5.2
  [opus] Claude Opus   anthropic/claude-opus-4-6
```

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L143-158 | §4.1 多 Agent 会话机制（@ 寻址语法、回答顺序、上下文） |
| PDD | L189-200 | §4.3 Slash 命令列表 |
| PDD | L439-446 | §5.2 输入交互（补全、历史） |
| TDD | L174-226 | §3.2 消息路由（Addressee 枚举、parse_input 实现） |
| TDD | L502-536 | §3.5 Slash 命令系统（枚举、from_input、执行） |
| TDD | L525-536 | §3.5.1 /agents 输出规格 |
