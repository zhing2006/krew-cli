## Context

目前系统有 `/agents`、`/mcp`、`/skills` 三个命令分别展示 agent、MCP 工具、skill 信息，但缺少一个统一展示每个 agent 可用的非 MCP runtime tools 的命令。每个 agent 拥有独立的 `Arc<ToolRegistry>`，其中混合注册了 built-in 工具、MCP 工具和可选的 `run_agent` sub-agent 工具。

## Goals / Non-Goals

**Goals:**
- 新增 `/tools` slash command，按 agent 分组显示所有非 MCP 的 runtime tools
- `tools=false` 的 agent 显示 `no tool(s)` 占位；初始化失败的 agent 显示 `unavailable`
- 每个工具显示名称和描述
- 显示风格与现有 `/agents`、`/mcp` 保持一致
- `/tools` 出现在 `/help` 列表和 tab 补全中

**Non-Goals:**
- 不显示 MCP 工具（已有 `/mcp`）
- 不显示工具的参数 schema
- 不做 agent 间工具集去重合并

## Decisions

### 1. 工具过滤策略
使用 `krew_tools::mcp::is_mcp_tool()` 负向过滤掉 MCP 工具（`mcp__` 前缀），保留所有其他 runtime tools。语义上定义为"非 MCP 的 runtime tools"，而非"仅 built-in + sub-agent"。这样未来新增非 MCP 动态工具时自然被包含，无需修改过滤逻辑。

**备选方案 A**: 维护一个 built-in 工具名称白名单 — 拒绝，因为新增工具时需要同步更新白名单，容易遗漏。
**备选方案 B**: 给 `ToolSpec` 加 `source` / `category` 字段做正向过滤 — 拒绝，当前工具种类只有 built-in / MCP / sub-agent 三类，加元数据是 overengineering。

### 2. Agent 遍历顺序
遍历 `self.config.agents`（即配置文件中的声明顺序），而非 `self.agents` HashMap。这与 `/agents` 命令保持一致，输出顺序可预测。

### 3. Agent 状态区分
遍历 `config.agents` 时，对每个 agent 检查 `self.agents.get(name)`：

- **存在 + registry 非空** → 正常显示工具列表，header 标注 `N tool(s)`
- **存在 + registry 为空** → header 标注 `no tool(s)`（合法的 tools=false）
- **不存在** → header 标注 `unavailable`（初始化失败，provider/API key 问题）

这避免了把"显式禁用工具"和"初始化失败"混为一谈。

### 4. 显示格式
```
Tools:
  [gpt]  GPT ─── 8 tool(s)
    read_file       Read file contents
    glob            File pattern matching
    ...

  [reader]  Reader ─── no tool(s)

  [broken]  Broken ─── unavailable
```
- Agent header: `[name]  DisplayName ─── N tool(s)` — agent 名称使用配置颜色
- 工具行: 4 空格缩进，名称左对齐 16 字符宽，描述用 DarkGray 色
- Agent 之间空一行分隔

## Risks / Trade-offs

- [工具描述来源] `ToolSpec.description` 可能较长 → 在终端宽度有限时自然截断，不做额外处理
- [性能] `specs()` 遍历 + MCP 过滤开销极小 → 无风险
