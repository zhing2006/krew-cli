## Context

krew 当前的 Agent 系统提示词仅来源于 `.krew/settings.toml` 中的 `system_prompt` 字段。用户无法通过项目级文件为 Agent 提供项目上下文（架构、约定、编码规范等）。主流 AI 编码工具（Claude Code、Codex、Copilot）均支持项目级指令文件自动加载。

当前代码结构：
- `krew-config` 负责加载 `.krew/settings.toml`
- `krew-core` 的 Agent Loop 在构建 LLM 请求时使用 `AgentConfig.system_prompt` 作为系统消息
- 指令文件的加载逻辑应归属 `krew-config`，注入逻辑归属 `krew-core`

## Goals / Non-Goals

**Goals:**
- krew 启动时自动发现并加载工作目录下的 `AGENTS.md` 文件内容
- 将指令文件内容注入到所有 Agent 的系统提示词中，与 `system_prompt` 配置合并
- 支持层级化加载：从工作目录向上遍历到根目录，合并所有找到的 `AGENTS.md`
- 更新 PDD 和 TDD 文档以描述此功能

**Non-Goals:**
- 不支持 per-agent 指令文件（如 `AGENTS-gpt.md`），v0.1 仅支持全局指令
- 不支持 `.krew/` 目录内的指令文件配置（指令文件放在项目目录，不在 `.krew/` 内）
- 不解析指令文件内的特殊语法（如条件指令、变量替换），纯文本注入
- 不支持通过配置文件指定自定义指令文件名

## Decisions

### 1. 文件名选择：`AGENTS.md`

使用 `AGENTS.md` 作为指令文件名。

**理由：**
- krew 本身就是一个"多 Agent 协作"工具，`AGENTS.md` 语义契合
- 与 OpenAI Codex 使用的文件名一致，用户可复用已有的指令文件
- 大写文件名符合项目根目录惯例（如 README.md、LICENSE）

**替代方案考虑：**
- `KREW.md` — 更品牌化，但缺乏行业认知
- `.krew/instructions.md` — 隐藏在配置目录中，不够显眼

### 2. 层级化加载策略

从工作目录（cwd）开始，逐级向上遍历父目录，收集所有找到的 `AGENTS.md` 文件。组装顺序：**祖先目录在前，子目录在后**（子目录内容可补充或覆盖祖先的通用指令）。

**理由：**
- monorepo 场景下，根目录放通用指令，子项目放特定指令
- 与 Claude Code 的 `CLAUDE.md` 层级加载机制一致

**停止条件：** 遇到文件系统根目录时停止。

### 3. 注入位置：system prompt 前置

指令文件内容拼接在 `system_prompt` **之前**，格式为：

```
<project-instructions>
{AGENTS.md 内容}
</project-instructions>

{原始 system_prompt}
```

**理由：**
- 项目级指令是通用背景信息，应先于 agent 个性化提示词
- 使用 XML 标签包裹，便于 LLM 理解指令边界
- `system_prompt` 放在后面，允许 agent 级配置覆盖或补充项目指令

### 4. 加载时机：App 初始化阶段

在 `App` 启动时（加载配置之后、创建 Agent 之前），调用 `krew-config` 的指令文件加载函数，将结果存储在运行时状态中。Agent 构建系统消息时从中读取。

**理由：**
- 一次加载，所有 Agent 共享，避免重复 I/O
- 文件不存在时静默跳过（不报错），这是可选功能

### 5. 文件大小限制

单个 `AGENTS.md` 文件限制最大 100KB。超出时截断并在末尾追加警告。

**理由：**
- 防止意外加载超大文件导致 token 浪费
- 100KB 足以容纳详尽的项目说明

## Risks / Trade-offs

- **[Token 消耗增加]** → 指令内容会增加每次 LLM 请求的 prompt tokens。通过文件大小限制（100KB）缓解，用户也可通过精简指令文件控制
- **[层级加载性能]** → 向上遍历目录可能跨越多层。实际影响极小（仅启动时一次性 I/O），且遇到根目录即停止
- **[文件编码]** → 假设 `AGENTS.md` 为 UTF-8 编码。非 UTF-8 文件会在读取时产生错误，此时跳过并输出警告日志
