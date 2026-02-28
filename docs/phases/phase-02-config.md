# Phase 2: 配置系统

> 目标：加载 `.krew/settings.toml`，解析 Agent/Provider/MCP 配置，CLI 参数覆盖。

## 实现内容

- **配置加载**：`krew-config` 读取 `.krew/settings.toml`，反序列化为 `Config` 结构体
- **配置覆盖**：CLI 参数 (`--agents`, `--approval-mode`, `--config`) 覆盖文件配置
- **AGENTS.md 加载**：已实现（`load_project_instructions`），集成到启动流程
- **启动 banner 更新**：显示实际加载的 Agent 列表（名称 + 颜色）
- **配置校验**：`reply_order` 中的 Agent 必须存在、Provider 引用合法、必填字段检查
- **错误提示**：配置文件不存在或格式错误时给出清晰提示

## 验收标准

```txt
$ cargo run
krew v0.1.0
Agents: [gpt] GPT-5.2 | [opus] Claude Opus | ...
（echo 模式继续工作）

$ cargo run --agents gpt,opus
Agents: [gpt] GPT-5.2 | [opus] Claude Opus
```

## 参考

| 文档 | 位置 | 内容 |
| ---- | ---- | ---- |
| PDD | L289-387 | §4.6 配置系统完整定义 |
| PDD | L300-387 | §4.6.2 配置文件结构（TOML 示例） |
| PDD | L389-417 | §4.6.3 AGENTS.md 项目级指令 |
| PDD | L493-517 | §6 命令行参数定义 |
| TDD | L686-766 | §3.7 配置管理（数据结构、加载优先级） |
| TDD | L768-823 | §3.7.3 AGENTS.md 加载与注入 |
| TDD | L1059-1063 | krew-config 源码结构 |
