## Context

当前 `krew-config` 的 `Config::load()` 只接受一个路径参数，加载单个 `.krew/settings.toml` 文件。所有配置（providers、agents、settings、mcp_servers）都在同一个文件中定义。这意味着用户在多个项目中使用相同的 API providers 时需要重复配置。

Commands discovery（`discover_commands`）只扫描 `.krew/commands/`，而 skills discovery（`discover_skills`）已经支持 `.krew/` 和 `.agents/` 两层，但两者不统一，也都缺少 `.claude/` 兼容。

关键约束：现有 `Config` / `Settings` 结构体的字段大多是非 `Option` 的（使用 `#[serde(default)]` 填充默认值），`Config::load()` 反序列化后已无法区分字段是"用户显式设置"还是"走了默认值"。这意味着不能在反序列化完成的 `Config` 上做 merge——必须在 raw/partial 阶段合并。

## Goals / Non-Goals

**Goals:**
- 支持 `~/.krew/settings.toml` 作为 user-level 配置，减少跨项目重复配置
- User config 与 project config 分层合并，project 优先
- Commands 和 skills 统一为相同的 6 路径 discovery 策略
- 兼容 `.claude/` 目录，方便从 Claude Code 迁移

**Non-Goals:**
- 不做 `.claude/settings.json` 的格式兼容（仅兼容 commands/skills 目录）
- 不做 user-level 的 `agents` / `reply_order` 定义（这些仅在 project config 中有意义）
- 不做配置继承链（只有 user + project 两层，不支持更多层级）
- 不做 workspace-level config（monorepo 多项目共享）

## Decisions

### Decision 1: 双 Partial 类型 + resolve 模型

**选择**: User config 和 project config 都解析为 partial 类型，合并后再 resolve 成最终 `Config`。

```
~/.krew/settings.toml  →  UserConfig  (全 Option，无 agents/reply_order)
.krew/settings.toml    →  RawConfig   (settings 字段为 Option，agents/reply_order 非 Option)
                              ↓ merge
                         RawConfig (合并后)
                              ↓ resolve (填充默认值)
                         Config (最终，非 Option 字段)
```

**替代方案 A**: 只有 UserConfig 是 partial，Config 保持不变。问题：Config::load() 反序列化后字段存在性已丢失，无法区分"显式设置"和"走默认值"。

**替代方案 B**: 把 Config 本身改成全 Option。问题：破坏所有下游消费者代码。

**理由**: 引入 `RawConfig` 作为中间表示，project config 的 settings 字段也用 `Option`，保留字段存在性信息。合并后由 `resolve()` 填充默认值生成最终 `Config`。现有 `Config` 类型和下游代码完全不变。

```rust
/// Project-level raw configuration — settings fields are Option to preserve
/// explicit-vs-default distinction during merge.
#[derive(Debug, Clone, Deserialize)]
pub struct RawConfig {
    #[serde(default)]
    pub settings: RawSettings,
    pub agents: Vec<AgentConfig>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub skills: Option<SkillsConfig>,
}

/// Raw settings — all fields Option to preserve explicit-vs-default.
/// reply_order is non-Option because it's project-only and always required.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RawSettings {
    pub approval_mode: Option<ApprovalMode>,
    pub reply_order: Vec<String>,  // project-only, not in UserConfig
    pub auto_compact_threshold: Option<u32>,
    pub compact_keep_rounds: Option<usize>,
    pub input_history_limit: Option<usize>,
    pub paste_burst_detection: Option<bool>,
    pub worker_threads: Option<usize>,
    pub other_agent_role: Option<OtherAgentRole>,
    pub retry: Option<RetryConfig>,
    pub shell_allow_commands: Option<Vec<String>>,
    pub fetch_allow_domains: Option<Vec<String>>,
    pub agent_to_agent_routing: Option<AgentToAgentRouting>,
    pub agent_to_agent_max_rounds: Option<u32>,
}

/// User-level configuration — all fields optional.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub settings: UserSettings,
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    #[serde(default)]
    pub skills: Option<SkillsConfig>,
}

/// User settings — same shape as RawSettings minus reply_order.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UserSettings {
    pub approval_mode: Option<ApprovalMode>,
    pub auto_compact_threshold: Option<u32>,
    // ... (same Option fields as RawSettings, except reply_order)
}
```

### Decision 2: 合并策略

`RawConfig` 新增 `merge_user(&mut self, user: &UserConfig)` 方法，在 resolve 之前调用。

合并规则：
- **settings 标量字段**: project `Some` 优先；project `None` 时用 user 的值（也可能是 `None`）
- **`providers` HashMap**: user 作为 base，project 的同名 key **整项替换** user 的
- **`mcp_servers` Vec**: user 的在前，project 的追加在后；同名 server 中 project 覆盖 user
- **`skills`**: project `Some` 时用 project 的；project `None` 时用 user 的。`extra_paths` 不做跨层追加——哪层定义了 skills 就用哪层的完整 skills 配置

providers 采用**整项替换**而非字段级 merge。理由：
- ProviderConfig 的 `type` 是必填字段，不能省略
- 字段级 merge 需要额外的 `PartialProviderConfig` 类型，复杂度高
- 实际场景中 project 覆盖 provider 通常是要切换到完全不同的配置（如换 base_url），整项替换更符合直觉
- 用户如果只想改一个字段，在 project config 中重复几行 provider 配置的成本很低

### Decision 3: resolve() 方法

`RawConfig` 新增 `pub fn resolve(self) -> Config`，将所有 `Option` 字段填充为默认值：

```rust
impl RawConfig {
    pub fn resolve(self) -> Config {
        Config {
            settings: Settings {
                approval_mode: self.settings.approval_mode.unwrap_or_default(),
                reply_order: self.settings.reply_order,
                auto_compact_threshold: self.settings.auto_compact_threshold,
                // ... each field unwrap_or(DEFAULT)
            },
            agents: self.agents,
            providers: self.providers,
            mcp_servers: self.mcp_servers,
            skills: self.skills.unwrap_or_default(),
        }
    }
}
```

现有 `Config::load()` 内部改为：解析 TOML → `RawConfig` → `.resolve()` → `Config`。这样不带 user config 时行为完全不变。

### Decision 4: 加载流程

```
main.rs load_config():
  1. UserConfig::load()                    // ~/.krew/settings.toml → UserConfig
  2. RawConfig::load(.krew/settings.toml)  // project config → RawConfig
  3. raw.merge_user(&user_config)          // 合并
  4. let config = raw.resolve()            // 填充默认值 → Config
  5. config.apply_cli_overrides(...)       // CLI 参数覆盖
  6. config.validate()                     // 校验（在 CLI overrides 之后！）
```

关键：**validate() 在 apply_cli_overrides() 之后**，保持与现有行为一致。现有代码就是先 `apply_cli_overrides` 再 `validate`（`main.rs:181-184`），`--agents` 可以过滤掉引用了不存在 provider 的 agent。

`--config PATH` 行为：显式指定时仍加载 user config 并合并。理由：user config 提供 providers/API keys 等基础设施，即使用了自定义 project config 也需要。

### Decision 5: UserConfig::load() 错误处理

**选择**: 文件不存在时静默返回 default。文件存在但解析失败时，**终端可见 warning**（通过 `eprintln!`）并返回 default。

**替代方案**: 解析失败时 fatal 退出。问题：user config 不是必须的，fatal 太激进。

**理由**: 用 `eprintln!` 而不是 `tracing::warn!`，因为日志写到文件用户看不到。终端上明确提示"~/.krew/settings.toml 解析失败，使用默认配置"，用户可以立即定位问题。

### Decision 6: Discovery 路径统一

在 `krew-core` 中新增共享函数 `discovery_paths(cwd, subdir) -> Vec<PathBuf>`：

```rust
pub fn discovery_paths(cwd: &Path, subdir: &str) -> Vec<PathBuf> {
    let mut paths = vec![
        cwd.join(".krew").join(subdir),
        cwd.join(".agents").join(subdir),
        cwd.join(".claude").join(subdir),
    ];
    if let Some(home) = dirs_home() {
        paths.push(home.join(".krew").join(subdir));
        paths.push(home.join(".agents").join(subdir));
        paths.push(home.join(".claude").join(subdir));
    }
    paths
}
```

- Commands: `discover_commands()` 签名改为 `discover_commands(cwd: &Path) -> CustomCommandRegistry`
- Skills: `discover_skills()` 内部路径构建替换为调用 `discovery_paths(cwd, "skills")`

### Decision 7: .claude 目录兼容范围

仅兼容 `.claude/commands/` 和 `.claude/skills/` 的文件读取。不兼容 `.claude/settings.json`、`CLAUDE.md` 等其他文件。`.claude/commands/` 中的文件格式与 `.krew/commands/` 完全一致（Markdown + 可选 YAML frontmatter），无需额外适配。

## Risks / Trade-offs

- **[Risk] RawConfig 引入额外类型** → 增加了代码量，但确保合并语义正确闭合。RawConfig 是内部类型，不影响公开 API
- **[Risk] providers 整项替换** → 用户想在 project 中只改 base_url 时需要重复写 type/api_key_env。但这比引入 PartialProviderConfig 简单得多，且实际场景中 provider 配置只有 3-5 行
- **[Risk] `.claude/commands/` 格式不完全兼容** → Claude Code 的 `$ARGUMENTS` 占位符在 krew 中已支持；其他 Claude 特有语法（如有）不做兼容，用户需手动调整
- **[Risk] UserConfig::load() 解析失败不 fatal** → 通过 eprintln! 终端可见 warning 缓解。如果后续发现不够，可以升级为 fatal
- **[Trade-off] `mcp_servers` 按 name 去重** → 如果用户在 user 和 project 中定义同名 MCP server，project 的会覆盖 user 的，这是有意设计
- **[Trade-off] User config 无 `agents`** → 用户无法定义全局默认 agents，但避免了复杂的合并逻辑
- **[Trade-off] skills 不做跨层 extra_paths 追加** → 简化合并逻辑，哪层定义了 skills 就用哪层的完整配置
