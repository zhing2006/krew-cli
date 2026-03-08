## Context

krew-cli 是一个多 AI agent 协作 CLI 工具。当前 agent 通过 `system_prompt` 获取静态指令，通过 MCP 和内置工具与外部系统交互。Agent Skills 是 Anthropic 发布的开放标准（2025 年 12 月），提供基于文件系统的轻量级方式让 agent 获取领域专用知识。该标准已被 26+ 平台采用（Claude Code、OpenAI Codex、Gemini CLI 等）。

当前系统已有：
- `AGENTS.md` 项目指令机制（加载到 system prompt）
- `ToolRegistry` + `ToolHandler` trait 的工具注册体系
- `read_file` 内置工具可读取文件
- `/skills` 命令已有 stub 但未实现

## Goals / Non-Goals

**Goals:**
- 符合 Agent Skills 开放标准规范（agentskills.io/specification）
- 实现三层渐进式加载：catalog（启动时）→ instructions（激活时）→ resources（按需）
- 支持项目级和用户级 skill 发现
- 提供 `activate_skill` 内置工具，让 LLM 自主决定何时激活 skill
- 实现 `/skills` 命令供用户查看可用 skills
- 在 system prompt 中注入 skill catalog
- 兼容 `.agents/skills/` 跨客户端约定和 `.krew/skills/` 专属路径

**Non-Goals:**
- 不实现 skill 远程注册中心或包管理
- 不实现 skill 的创建/编辑/发布工具
- 不实现子 agent 委托执行 skill（subagent delegation）
- 不实现上下文压缩时的 skill 内容保护（当前无上下文压缩机制）
- 不实现 `allowed-tools` 字段的预授权机制（标准中标记为 experimental）
- 不实现用户通过斜杠命令直接激活 skill（首期仅 LLM 驱动激活）

## Decisions

### D1: Skill 发现放在 krew-core 而非 krew-config

**决定**: 在 `krew-core` 中新增 `skill` 模块负责 skill 发现和解析。

**理由**: `krew-config` 负责 TOML 配置解析，而 skill 发现涉及文件系统扫描和 YAML+Markdown 解析，属于运行时行为。skill 配置（路径、信任）仍在 `krew-config` 中定义。

**替代方案**: 放在 `krew-tools` 中 — 但 skills 不是工具，而是知识/指令，语义上更适合 core。

### D2: YAML frontmatter 解析使用 serde_yaml

**决定**: 在 `krew-core` 的 `Cargo.toml` 中新增 `serde_yaml` 依赖，用于解析 SKILL.md 的 YAML frontmatter。

**理由**: 项目已使用 `serde` 体系，`serde_yaml` 是成熟的 YAML 解析库。frontmatter 格式简单（不超过 10 个字段），不需要完整的 YAML 解析器。

**替代方案**: 手工字符串解析 — 更轻量但不易扩展，且不处理 edge case（如冒号转义）。

### D3: activate_skill 工具作为只读内置工具

**决定**: 新增 `ActivateSkillTool` 实现 `ToolHandler` trait，注册为只读工具（`requires_approval() = false`），接受 skill name 参数，返回 SKILL.md body 内容 + 资源文件列表。

**理由**:
- 只读工具无需用户审批，LLM 可自主激活
- 专用工具比 `read_file` 更优：可控制返回内容（剥离 frontmatter、包装 XML 标签、列出资源文件）
- 可约束 name 参数为合法 skill 名称，防止 LLM 幻觉

**替代方案**: 依赖 `read_file` 激活（更简单但控制力差，无法添加元数据包装）。

### D4: Skill catalog 注入到 system prompt

**决定**: 在 agent 的 system prompt 构建过程中，将 skill catalog 作为独立段落注入，位于 project-instructions 之后、agent identity 之前。

**格式**:
```xml
<available-skills>
  <skill name="pdf-processing" location="/path/to/pdf-processing/SKILL.md">
    Extract text and tables from PDF files, fill forms, merge documents.
  </skill>
</available-skills>
```

附带简短行为指令告知 LLM 何时及如何激活 skill。

**理由**: system prompt 注入是最简单、兼容性最好的方式。每个 skill 仅 ~50-100 tokens。

### D5: 扫描路径和优先级

**决定**: 默认扫描以下路径，项目级优先于用户级（同名时项目级覆盖）：

| 优先级 | 范围 | 路径 |
|--------|------|------|
| 1（最高）| 项目 | `<cwd>/.krew/skills/` |
| 2 | 项目 | `<cwd>/.agents/skills/` |
| 3 | 用户 | `~/.krew/skills/` |
| 4（最低）| 用户 | `~/.agents/skills/` |

用户可通过 `settings.toml` 的 `[skills]` 配置节添加额外路径或禁用默认路径。

**理由**: 遵循 Agent Skills 标准推荐的 `.agents/skills/` 跨客户端约定，同时支持 `.krew/skills/` 专属路径。项目级优先是标准中的通用惯例。

### D6: SkillRecord 存储结构

**决定**: 发现阶段在内存中构建 `HashMap<String, SkillRecord>`（name → record），每个 record 存储：

```rust
struct SkillRecord {
    name: String,
    description: String,
    location: PathBuf,     // SKILL.md 的绝对路径
    base_dir: PathBuf,     // skill 目录的绝对路径
    compatibility: Option<String>,
    metadata: Option<HashMap<String, String>>,
}
```

Body 内容不预加载，激活时按需读取。

### D7: 宽容验证策略

**决定**: 遵循标准建议的宽容策略：
- name 不匹配目录名 → 警告，仍加载
- name 超过 64 字符 → 警告，仍加载
- description 缺失或为空 → 跳过该 skill，记录错误
- YAML 完全不可解析 → 跳过该 skill，记录错误

## Risks / Trade-offs

- **[不信任项目 skill]** → 首期不做信任检查，所有发现的 skill 都加载。后续可添加信任机制（如首次加载时提示用户确认）。
- **[大量 skills 增加 system prompt]** → 每个 skill catalog 条目 ~50-100 tokens，20 个 skill 约 1000-2000 tokens，可接受。若未来需要，可通过配置限制 skill 数量。
- **[YAML 解析依赖]** → 新增 `serde_yaml` 依赖。该 crate 成熟且广泛使用，风险低。
- **[skill 激活消耗 token]** → 单个 skill body 推荐 <5000 tokens。依赖 skill 作者遵守最佳实践。
- **[Windows 路径兼容]** → 使用 `PathBuf` 和 `std::fs` 标准库，跨平台兼容。扫描时统一使用 `/` 分隔符显示。
