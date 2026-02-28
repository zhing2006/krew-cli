# Phase 7: 会话持久化

> 目标：会话实时保存到 TOML 文件，支持 `/new` 新建和 `/resume` 恢复。

## 实现内容

- **会话存储**：`krew-storage` 实现 `save_session()` / `load_session()` / `list_sessions()`
- **TOML 文件格式**：按 TDD §3.6.1 定义的结构，每个会话一个 `.toml` 文件
- **实时持久化**：每条消息（用户消息 + Agent 回复）实时追加写入
- **/new 命令**：保存当前会话 → 清空上下文 → 开始新会话
- **/resume 命令**：列出历史会话（按时间倒序）→ 用户选择 → 加载消息历史
- **会话元数据**：ID、创建时间、最后活跃时间、Agent 列表、工作目录、累计 token
- **启动行为**：程序启动时自动创建新会话（或通过 `--resume` 恢复指定会话）

## 验收标准

```txt
$ cargo run
krew v0.1.0 — 新会话 a1b2c3d4
you> @gpt hello
[gpt] ...
you> /quit

$ cargo run
you> /resume
  [1] 2026-02-28 14:30 (gpt, opus) "hello"
  [2] 2026-02-27 09:15 (gpt) "test"
选择会话: 1
已恢复会话 a1b2c3d4

$ cat .krew/sessions/a1b2c3d4.toml  # 内容符合 TDD §3.6.1 格式
```

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L33-34 | §2.1 Session 概念（持久化、/resume、/new） |
| PDD | L111-123 | US-4 恢复历史会话 |
| PDD | L239-253 | §4.5.1-4.5.2 自动持久化 + Token 追踪 |
| PDD | L263-280 | §4.5.4-4.5.6 会话元数据、/resume 流程、/new 流程 |
| TDD | L618-667 | §3.6.1 TOML 文件存储格式（完整示例） |
| TDD | L670-677 | §3.6.2 存储路径结构 |
| TDD | L829-892 | §4 数据模型（Session 结构体） |
| TDD | L1053-1058 | krew-storage 源码结构 |
