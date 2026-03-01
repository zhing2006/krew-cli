## Context

krew-cli 已完成 Phase 1（TUI Echo 模式），具备日志、inline viewport、多行输入和 echo 回显功能。配置相关的数据类型（`Config`、`AgentConfig`、`ProviderConfig` 等）已在 `krew-config` 中定义，`AGENTS.md` 加载也已实现。但当前没有从 `.krew/settings.toml` 加载配置的功能，App 中也没有使用实际的 Agent 信息。

现有基础：
- `krew-config/src/lib.rs`: 所有配置结构体已定义（`Config`、`Settings`、`AgentConfig` 等），均派生了 `Deserialize`
- `krew-config/src/defaults.rs`: 仅定义了 `DEFAULT_AUTO_COMPACT_THRESHOLD` 常量
- `krew-config/src/instructions.rs`: `load_project_instructions()` 已实现并通过测试
- `krew-cli/src/main.rs`: CLI 参数已用 clap 定义（`--config`、`--agents`、`--approval-mode`），但未使用
- `krew-cli/src/app.rs`: `App` 仅存储 `cwd` 和 `project_instructions`，不持有 `Config`
- `krew-cli/src/render.rs`: 启动 banner 为硬编码信息，状态栏显示硬编码的 `suggest` 和 `auto-compact off`

## Goals / Non-Goals

**Goals:**
- 实现配置文件加载：从 `.krew/settings.toml` 读取 → 反序列化为 `Config`
- 提供内置默认配置，使用户无需配置文件即可启动（echo 模式）
- 实现 CLI 参数覆盖配置
- 配置加载后进行校验（Agent/Provider 引用完整性）
- 启动 banner 和状态栏展示实际配置信息
- 提供示例配置文件 `config.example.toml`

**Non-Goals:**
- 不在本阶段初始化 LLM client 或建立网络连接（Phase 4+）
- 不在本阶段实现 MCP server 启动（Phase 10）
- 不实现配置热重载
- 不实现环境变量解析来获取 API key（Phase 4 时 LlmClient 实现中处理）

## Decisions

### D1: 配置加载入口 — `Config::load(path)` 静态方法

在 `Config` 上实现 `pub fn load(path: &Path) -> Result<Config, ConfigError>`。

**理由**: 加载逻辑与 `Config` 类型紧密关联，静态方法是 Rust 惯用模式。`ConfigError` 使用 `thiserror` 定义，覆盖 IO 错误和反序列化错误。

**备选方案**: 独立函数 `load_config(path)` — 但静态方法在调用方更直观（`Config::load(&path)`）。

### D2: 内置默认配置 — `Config::default()` 提供零配置启动

实现 `Default for Config`，返回一个包含单个 echo agent 的最小配置。这样在 `.krew/settings.toml` 不存在时，程序仍可启动。

默认配置包含：
- `settings.approval_mode = Suggest`
- `settings.reply_order = ["echo"]`
- 单个名为 `echo` 的 agent（内置 echo 模式，不需要 provider）
- 空的 `providers` 和 `mcp_servers`

**理由**: 用户首次使用时无需创建配置文件，与 Phase 1 的 echo 模式保持兼容。

### D3: CLI 覆盖 — `Config::apply_cli_overrides()` 方法

在 `Config` 上实现 `pub fn apply_cli_overrides(agents: Option<&str>, approval_mode: Option<&str>) -> Result<(), ConfigError>` 方法：
- `--agents gpt,opus`: 过滤 `config.agents`，仅保留名称匹配的 agent，同时更新 `reply_order`
- `--approval-mode suggest`: 覆盖 `config.settings.approval_mode`

**理由**: 将覆盖逻辑集中在 Config 上，避免分散在 main.rs 中。

### D4: 配置校验 — `Config::validate()` 方法

在 `Config` 上实现 `pub fn validate(&self) -> Result<(), ConfigError>`，检查：
1. `reply_order` 中引用的每个 agent name 必须在 `agents` 中存在
2. 每个 agent 的 `provider` 必须在 `providers` 中存在
3. `agents` 中不能有重复的 `name`

**理由**: 在启动早期发现配置错误，提供明确的错误信息，比运行时 panic 更好。

### D5: 错误类型 — `ConfigError` 枚举

`krew-config` 作为 library crate 使用 `thiserror`（遵循项目约定），定义 `ConfigError`：
- `Io(std::io::Error)` — 文件读取失败
- `Parse(toml::de::Error)` — TOML 反序列化失败
- `Validation(String)` — 校验错误（含具体描述）

**理由**: 结构化错误允许调用方（krew-cli）针对不同错误类型给出不同提示。

**注意**: 需要启用 toml crate 的 `serde` feature 来使 `toml::de::Error` 实现 `std::error::Error`（用于 `#[from]`）。

### D6: 启动流程变更 — main.rs 中集成配置加载

修改 `main()` 流程：
1. 解析 CLI 参数
2. 初始化日志
3. **加载配置**：按 `--config` 路径或默认 `.krew/settings.toml` 加载。`--config` 显式指定的路径不存在时报错退出；默认路径不存在时静默使用 `Config::default()`
4. **应用 CLI 覆盖**
5. **校验配置**
6. 将 `Config` 传入 `App::new(cwd, config)`

配置加载在终端初始化之前完成，这样错误信息能正常打印到 stderr。

### D7: 启动 banner 显示 Agent 列表

Banner 为 3 行内容（含边框共 5 行）：
```
╭──────────────────────────────────────────────────────────╮
│ >_ Krew CLI (v0.1.0)                                    │
│ Agents: [gpt] GPT-5.2 | [opus] Claude Opus              │
│ Directory: H:\path\...  Type /help for commands          │
╰──────────────────────────────────────────────────────────╯
```
- 第三行左对齐 `Directory:` + 路径，右对齐 `Type /help for commands`
- 路径超过可用宽度时用 `shorten_path()` 中间截断为 `...`
- 每个 agent 的 `[name]` 使用其配置的 `color` 着色

### D8: 状态栏显示配置信息

更新 `render::render_status_bar()` 从 `App.config` 读取：
- 左侧显示实际的 `approval_mode`
- 显示 `auto-compact` 阈值（如 `120k`）或 `auto-compact off`

### D9: 消息前缀样式

用户消息使用 `> `（绿色粗体），agent 回复使用 `● `（使用 agent 配置的颜色）。`insert_message()` 接收 `color_name` 参数，从 agent config 获取颜色。多行消息续行按 `unicode-width` 计算的前缀显示宽度对齐。

### D10: CJK 宽字符渲染

`custom_terminal::draw_cells()` 使用 `unicode-width` crate 计算每个 cell 的显示宽度。宽字符（CJK、emoji）在 Buffer 中占多个 cell，但只有首个 cell 包含实际字符，后续续接 cell 在绘制时跳过，避免覆盖宽字符的右半部分。`unicode-width` 作为 workspace 依赖统一管理。

## Risks / Trade-offs

- **[Risk] toml crate feature 兼容性** → workspace `Cargo.toml` 中 toml 已启用 `serde` feature，`toml::de::Error` 的 `#[from]` 正常工作。
- **[Risk] 配置文件路径在不同 OS 上的差异** → 使用 `Path` API 处理路径，不硬编码分隔符。
- **[Trade-off] 默认 echo agent 是否应出现在配置中** → 选择内置硬编码而非写入 settings.toml，避免自动创建用户未预期的文件。
- **[Trade-off] `Config.providers` 加 `#[serde(default)]`** → 允许用户省略空的 providers 表，对默认 echo 模式友好。
