## Context

krew-cli 当前的 Agent 每次会话都从零开始，没有跨会话的记忆能力。参考 free-code（Claude Code 复刻版）的 Memory 系统设计，我们需要在多 Agent 场景下实现持久化记忆。

与 free-code 的单 Agent 场景不同，krew-cli 的核心挑战在于：多个 Agent 共存于同一会话，需要区分"所有 Agent 共享的事实"和"特定 Agent 的行为偏好"。

当前 system prompt 构建流程（`build_system_prompt()`）：
```
Project Instructions → Skill Catalog → Sub-Agent Catalog → Agent Prompt
```

## Goals / Non-Goals

**Goals:**
- 实现两层 Memory 存储：Global（全 Agent 共享）+ Per-Agent（Agent 独立）
- 每次 agent turn 时自动加载记忆内容，注入 system prompt
- Agent 通过已有的 `read_file` / `write_file` 工具主动读写记忆
- 记忆文件为纯 Markdown 格式，MEMORY.md 作为索引
- 实施 MEMORY.md 大小限制防止 prompt 膨胀
- `.krew/memory/**` 路径豁免 DANGEROUS_DIRECTORIES 审批

**Non-Goals:**
- 不做后台自动提取（free-code 的 forked agent 模式依赖 prompt cache 共享，多 provider 场景下不适用）
- 不做 `findRelevantMemories` AI 相关性筛选（v1 靠 Agent 自行 `read_file` 读取感兴趣的记忆 topic 文件）
- 不做记忆文件内容解析（无 frontmatter 解析、无目录扫描、无文件数限制的系统端强制）
- 不添加 `/remember` 等 slash command
- 不添加 `settings.toml` 配置项
- 不做 Session Memory（会话内临时记忆 / auto-compaction）

## Decisions

### Decision 1: 两层存储——Global + Per-Agent

**选择**：记忆按类型自动归属到两个层次

```
.krew/memory/                          ← Global 层
├── MEMORY.md                          ← 全局索引
├── user_role.md
├── project_deadline.md
└── agents/                            ← Per-Agent 层
    └── {agent_name}/
        ├── MEMORY.md                  ← Agent 索引
        └── feedback_no_emoji.md
```

归属规则（由 prompt 指令引导 Agent 行为）：
- `user` / `project` / `reference` → Global（所有 Agent 可见）
- `feedback` → Per-Agent（仅该 Agent 可见）

**理由**：用户身份、项目事实、外部链接对所有 Agent 一样，但行为反馈（"别用 emoji"、"多写注释"）是针对特定 Agent 的。这比纯粹的每 Agent 独立记忆更合理——避免用户重复告诉每个 Agent 同样的背景信息。

**备选方案**：
- 纯 Per-Agent：简单但需重复输入 → 放弃
- 纯 Global：feedback 类型会冲突 → 放弃
- Per-Agent + 自动同步：需要合并策略，过于复杂 → 放弃

### Decision 2: 仅 System Prompt 主动写入，不做后台提取

**选择**：在 system prompt 中注入 Memory 读写指令，Agent 在对话过程中通过 `write_file` / `read_file` 主动管理记忆。

**理由**：
- free-code 的后台提取依赖 prompt cache 共享（forked agent 复用父请求的缓存前缀，98%+ 命中率）。krew-cli 使用多种 provider（OpenAI / Anthropic / Google），各家 cache 机制不同，无法统一复用。
- 后台提取意味着每轮对话额外一次 API 调用，在多 Agent 场景下成本倍增。
- 主动写入零额外 API 成本，利用已有的 tool call 循环。

**备选方案**：
- 会话结束时用指定 Agent 提取：仍有额外 API 成本，延迟退出 → 放弃
- 用最便宜的 model 做提取：增加配置复杂度 → 放弃

### Decision 3: 不做 findRelevantMemories（v1）

**选择**：v1 不实现 AI 驱动的记忆相关性筛选。Agent 在 system prompt 中看到 MEMORY.md 索引标题，需要时自行调用 `read_file` 读取 topic 文件完整内容。

**理由**：
- free-code 的 `findRelevantMemories` 需要一次额外的 sideQuery API 调用（用 Sonnet 做筛选）。多 provider 场景下选择哪个 model 做 sideQuery 是个配置问题。
- 早期记忆数量不多时，MEMORY.md 索引本身很短，Agent 用 `read_file` 读感兴趣的 topic 文件即可。
- 记忆文件不需要 frontmatter（frontmatter 的 description 字段本是给 `scanMemoryFiles` 做 manifest 用的），纯 Markdown 更简单。

**未来演进**：当记忆积累到几十上百条时，可以考虑加入 `findRelevantMemories` + frontmatter 机制。

### Decision 4: Agent Identity 用 `name` 字段

**选择**：Per-Agent 目录用 `AgentConfig.name` 作为目录名。重命名 Agent 不迁移已有记忆，这是可接受的行为。

**理由**：`name` 是 Agent 的唯一标识符（用于 @addressing），用户一般不会随便改名。比 `provider+model` 组合更简洁、更稳定。

### Decision 5: 记忆注入位置在 Agent Prompt 之前

**选择**：在 `build_system_prompt()` 中，Memory 内容注入在 Sub-Agent Catalog 之后、Agent Prompt 之前：

```
Project Instructions → Skill Catalog → Sub-Agent Catalog → 【Memory】 → Agent Prompt
```

**理由**：
- Memory 内容是动态的、可能较长的，放在最后（Agent Prompt 之前）可以避免影响前面的 cache 前缀
- Agent Prompt 是用户自定义的最高优先级指令，应该在最后以便覆盖

### Decision 6: MEMORY.md 大小限制对齐 free-code

**选择**：

| 项目 | 值 |
|------|------|
| 记忆文件格式 | 纯 Markdown（无 frontmatter） |
| MEMORY.md 格式 | 纯索引，每行一条 `- [Title](file.md) — hook` |
| MEMORY.md 最大行数 | 200 |
| MEMORY.md 最大字节 | 25,000 |

系统仅截断 MEMORY.md 索引加载，不扫描/计数/裁剪 topic 文件。记忆文件数量由 prompt 指令中的"保持索引精简"引导 Agent 自律。

**理由**：free-code 的 `MAX_MEMORY_FILES = 200` 也只是 scan 时截断（`memoryScan.ts:73`），没有主动删文件的机制。我们不做 scan，所以不需要这个限制的系统端实现。

### Decision 7: `.krew/memory/**` 的 Approval Carve-out

**选择**：在 `approval.rs` 的 bypass immunity 检查（Step 1）中，对匹配 `.krew/memory/` 前缀的路径提前返回 `Approved`，跳过 DANGEROUS_DIRECTORIES 检查。

```rust
// Step 1 之前（Step 0 deny rules 之后）:
if is_memory_path(&normalized) {
    return ToolApproval::Approved;
}
```

**理由**：free-code 的做法完全一样（`filesystem.ts:1565-1581`）：memory 路径默认在 `~/.claude/` 下（属于 DANGEROUS_DIRECTORIES），通过 carve-out 豁免。注释明确写道 "This pre-safety-check carve-out exists because the default path is under ~/.claude/, which is in DANGEROUS_DIRECTORIES."

krew-cli 的 `.krew/memory/` 面临同样的问题——`.krew` 在 `DANGEROUS_DIRECTORIES` 列表中。不加 carve-out，Agent 每次读写记忆都会弹审批提示，完全不可用。

**安全保证**：
- Carve-out 在 deny rules（Step 0）之后，用户仍可通过 deny_rules 禁止
- 仅限 `.krew/memory/` 子路径，`.krew/settings.toml` 等仍受保护
- Memory 目录不含敏感配置，安全风险极低

### Decision 8: tools=false Agent 只注入只读 Memory

**选择**：当 `AgentConfig.tools = false` 时，`load_memory_prompt()` 仅注入 MEMORY.md 索引内容（带标题），不注入写入指令模板。

**理由**：tools=false 的 Agent 没有 `read_file` / `write_file` 可用，注入写入指令会造成困惑——Agent 被告知"可以保存记忆"但实际无法执行。只注入索引内容让 Agent 了解已有记忆上下文，但不期望其写入。

### Decision 9: 新增 `memory` 模块在 `krew-core` 中

**选择**：在 `krew-core` 中新增 `memory` 模块，负责：
- `load_memory_prompt(agent_name, cwd, has_tools)` → 构建完整的 Memory system prompt 段
- `read_and_truncate(path, max_lines, max_bytes)` → 读取 MEMORY.md 并截断
- `is_memory_path(path, cwd)` → 判断路径是否在 `.krew/memory/` 下（供 approval.rs 调用）
- Memory 指令模板常量

不创建独立 crate，因为 Memory 功能与 agent loop / system prompt 紧密耦合，且代码量不大。

### Decision 10: 记忆在每次 Agent Turn 时加载

**选择**：MEMORY.md 内容在每次 `start_completion()` 调用时通过 `build_system_prompt()` 重新加载。

**理由**：`build_system_prompt()` 在 `start_completion()` 内部调用（`agent/mod.rs:104`），每次 agent turn 都会重建 system prompt。Memory 加载自然跟随这个流程，无需额外的缓存机制。这意味着如果前一个 Agent 在本轮写入了新记忆，后续 Agent 的 `start_completion()` 会加载到最新的 MEMORY.md 内容。

## Risks / Trade-offs

### Risk 1: Agent 写入记忆消耗 tool call 轮次
Agent 写记忆需要调用 `write_file`（写文件 + 更新索引 = 2 次 tool call），在 max_rounds 限制下可能影响主任务。
→ **缓解**：max_rounds 默认 25，记忆写入只占 2 轮。且 Agent 通常在回答完主问题后才写记忆。

### Risk 2: 多 Agent 对同一个 MEMORY.md 的覆盖写
当 @all 时多个 Agent 串行执行（按 reply_order）。后一个 Agent 通过 `write_file` 写 MEMORY.md 时，如果完全覆盖而非追加，可能丢失前一个 Agent 刚添加的条目。
→ **缓解**：这是可接受的竞争条件。串行执行意味着不存在真正的并发写入。如果后一个 Agent 先 `read_file` 再 `write_file`，能看到前一个 Agent 的修改。最坏情况是某个 Agent 直接覆盖写（不先读），丢失部分索引条目，用户下次对话时可修复。

### Risk 3: 不同 LLM 对 Memory 指令的遵循程度不同
某些 model 可能忽略 memory 指令或格式不正确。
→ **缓解**：Memory 是 best-effort 机制，格式错误不会影响核心功能。指令保持简洁清晰以提高跨 model 兼容性。

### Risk 4: Memory 目录不存在时 write_file 失败
首次使用时 `.krew/memory/` 和 `.krew/memory/agents/{name}/` 目录不存在。
→ **缓解**：在 Memory 指令中明确写"目录已存在"，在 `load_memory_prompt()` 中确保创建目录（`fs::create_dir_all`）。
