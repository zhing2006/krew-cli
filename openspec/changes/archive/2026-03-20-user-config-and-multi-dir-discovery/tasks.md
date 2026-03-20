## 1. Raw/Partial 类型定义 (krew-config)

- [x] 1.1 定义 `RawSettings` 结构体（所有标量字段 `Option`，`reply_order` 为 `Vec<String>`，派生 `Deserialize`/`Clone`/`Debug`/`Default`）
- [x] 1.2 定义 `RawConfig` 结构体（`settings: RawSettings`、`agents`、`providers`、`mcp_servers`、`skills: Option<SkillsConfig>`）
- [x] 1.3 定义 `UserSettings` 结构体（同 `RawSettings` 但无 `reply_order`）
- [x] 1.4 定义 `UserConfig` 结构体（`settings: UserSettings`、`providers`、`mcp_servers`、`skills: Option<SkillsConfig>`，无 `agents`）
- [x] 1.5 定义 `USER_CONFIG_DIR` 常量（值 `".krew"`）
- [x] 1.6 公开导出 `RawConfig`、`RawSettings`、`UserConfig`、`UserSettings`、`USER_CONFIG_DIR`

## 2. resolve + load 重构 (krew-config)

- [x] 2.1 实现 `RawConfig::resolve(self) -> Config`——所有 Option 字段 unwrap_or(default)
- [x] 2.2 实现 `RawConfig::load(path) -> Result<RawConfig, ConfigError>`
- [x] 2.3 重构 `Config::load()` 内部：先 `RawConfig::load()` 再 `.resolve()`，保持外部行为不变
- [x] 2.4 实现 `RawConfig::default()`——空 agents、空 providers，settings 全 None
- [x] 2.5 测试: resolve 将 None 字段填充为默认值（approval_mode → Suggest，worker_threads → 4，等）
- [x] 2.6 测试: resolve 保留 Some 字段的值不变
- [x] 2.7 测试: Config::load() 重构后外部行为不变（有效文件、文件不存在、格式错误）
- [x] 2.8 测试: RawConfig 反序列化保留字段存在性（只设 approval_mode 时其余为 None）

## 3. UserConfig 加载 (krew-config)

- [x] 3.1 实现 `UserConfig::load() -> UserConfig`（从 `~/.krew/settings.toml` 加载，文件不存在返回 default，解析失败 `eprintln!` warning 并返回 default）
- [x] 3.2 测试: user config 文件存在且合法时返回正确解析结果
- [x] 3.3 测试: user config 文件不存在时返回 UserConfig::default()
- [x] 3.4 测试: user config TOML 格式错误时返回 default 并输出 warning

## 4. Config 合并逻辑 (krew-config)

- [x] 4.1 实现 `RawConfig::merge_user(&mut self, user: &UserConfig)` — 完整合并逻辑
- [x] 4.2 测试: providers — user 定义 openai + anthropic，project 只定义 openai → 合并后两个都有，openai 用 project 的
- [x] 4.3 测试: providers — user 定义 provider，project 不定义任何 → 合并后包含 user 的
- [x] 4.4 测试: providers — 双方均空 → 合并后空
- [x] 4.5 测试: mcp_servers — user 定义 [A, B]，project 定义 [B, C] → 合并后 [A, B(project), C]，B 用 project 的
- [x] 4.6 测试: mcp_servers — user 定义 [A]，project 为空 → 合并后 [A]
- [x] 4.7 测试: settings 标量 — project Some 优先于 user Some
- [x] 4.8 测试: settings 标量 — project None 时继承 user Some
- [x] 4.9 测试: settings 标量 — 双方均 None → 保持 None（由 resolve 填默认值）
- [x] 4.10 测试: skills — project Some 优先于 user Some
- [x] 4.11 测试: skills — project None 时继承 user Some
- [x] 4.12 测试: 端到端 — user config + project config → merge → resolve → 验证最终 Config 所有字段正确

## 5. 统一 Discovery 路径 (krew-core)

- [x] 5.1 新增 `discovery_paths(cwd: &Path, subdir: &str) -> Vec<PathBuf>` 公开函数
- [x] 5.2 测试: 完整路径列表（6 个路径按正确优先级排列）
- [x] 5.3 测试: home 不可用时仅返回 3 个 project-level 路径
- [x] 5.4 测试: subdir 参数为 "commands" 和 "skills" 时路径正确

## 6. Commands 多目录 Discovery (krew-core)

- [x] 6.1 修改 `discover_commands` 签名为 `discover_commands(cwd: &Path) -> CustomCommandRegistry`
- [x] 6.2 内部调用 `discovery_paths(cwd, "commands")` 扫描所有路径
- [x] 6.3 实现 first-found wins 同名命令优先级逻辑
- [x] 6.4 测试: .krew/commands/ 中的命令正常发现（现有测试适配）
- [x] 6.5 测试: .agents/commands/ 中的命令正常发现
- [x] 6.6 测试: .claude/commands/ 中的命令正常发现
- [x] 6.7 测试: 同名命令 .krew 优先于 .agents 优先于 .claude
- [x] 6.8 测试: 多目录命令合并（各目录不同名的命令全部注册）
- [x] 6.9 测试: 所有目录均不存在时返回空 registry

## 7. Skills 多目录 Discovery (krew-core)

- [x] 7.1 修改 `discover_skills` 内部路径构建，使用 `discovery_paths(cwd, "skills")` 替换硬编码路径
- [x] 7.2 测试: .claude/skills/ 中的 skill 正常发现
- [x] 7.3 测试: .krew > .agents > .claude 优先级（同名 skill first-found wins）
- [x] 7.4 测试: project level 优先于 user level
- [x] 7.5 测试: 多目录 skill 合并（各目录不同名的 skill 全部发现）

## 8. CLI 集成 (krew-cli)

- [x] 8.1 修改 `load_config()` 流程：UserConfig::load() → RawConfig::load() → merge_user → resolve → apply_cli_overrides → validate
- [x] 8.2 修改 `App::new()` 中 `discover_commands` 调用，传入 `cwd` 参数
- [x] 8.3 user config 成功加载时记录 `tracing::info!`（仅日志文件，不在终端显示）
- [x] 8.4 验证 --config PATH 仍然加载 user config 并合并
- [x] 8.5 验证 validate() 在 apply_cli_overrides() 之后（--agents 能过滤掉坏 agent）

## 9. 文档更新

- [x] 9.1 更新 `config.example.toml` 添加 user config 说明注释
- [x] 9.2 更新 PDD 中配置章节，补充 user-level 配置和多目录 discovery 说明
- [x] 9.3 更新 TDD 中配置相关章节
