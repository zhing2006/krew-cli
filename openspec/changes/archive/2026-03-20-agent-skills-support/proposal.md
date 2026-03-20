## Why

Agent Skills 是 Anthropic 于 2025 年 12 月发布的开放标准，已被 Claude Code、OpenAI Codex、Gemini CLI、GitHub Copilot、Cursor 等 26+ 平台采用。它提供了一种轻量级、基于文件系统的方式，让 AI agent 获得领域专用知识和工作流。与 MCP（提供外部工具连接）互补，Skills 教会 agent "怎么做"。krew-cli 作为多 agent 协作 CLI 工具，支持这一行业标准将显著提升 agent 的专业能力和用户体验。

## What Changes

- 新增 skill 发现机制：启动时扫描项目级和用户级目录，查找包含 `SKILL.md` 的子目录
- 新增 `SKILL.md` 解析器：提取 YAML frontmatter（name、description 等）和 Markdown body
- 新增 skill catalog 注入：在 agent 的 system prompt 中注入可用 skills 列表（仅 name + description）
- 新增 `activate_skill` 内置工具：LLM 可通过工具调用激活 skill，加载完整指令到上下文
- 实现 `/skills` 斜杠命令：列出当前可用的所有 skills
- 新增配置项：支持在 `settings.toml` 中配置 skill 扫描路径和信任级别
- skill 目录中的 scripts/references/assets 可通过现有的 `read_file` 工具按需访问

## Capabilities

### New Capabilities
- `skill-discovery`: 启动时扫描文件系统发现可用 skills，解析 SKILL.md frontmatter，处理名称冲突和优先级
- `skill-activation`: 提供 activate_skill 内置工具，支持 LLM 驱动的 skill 激活，将 skill 指令注入对话上下文
- `skill-catalog`: 在 system prompt 中构建和注入 skill 目录（name + description 列表），实现渐进式信息披露
- `skill-config`: settings.toml 中的 skill 相关配置（扫描路径、信任设置），以及 /skills 命令实现

### Modified Capabilities
- `config-types`: 新增 skill 相关配置字段（扫描路径、信任级别）
- `builtin-tools-readonly`: 新增 activate_skill 作为只读内置工具
- `slash-commands`: 实现已有的 `/skills` 命令 stub
- `project-instructions`: system prompt 构建流程需注入 skill catalog

## Impact

- **krew-config**: 新增 `SkillConfig` 结构体和相关配置解析
- **krew-tools**: 新增 `activate_skill` 工具实现，新增 `skill` 模块用于发现和解析
- **krew-core**: 修改 system prompt 构建逻辑注入 skill catalog；agent 初始化时注册 skill 工具
- **krew-cli**: 实现 `/skills` 命令的 TUI 展示
- **依赖**: 无新外部依赖（YAML frontmatter 解析可复用现有 serde/yaml 能力）
- **配置文件**: `settings.toml` 新增可选 `[skills]` 配置节
- **文件系统**: 读取 `.krew/skills/`、`.agents/skills/` 等目录
