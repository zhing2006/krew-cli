## 1. 配置层（krew-config）

- [x] 1.1 在 `krew-config/src/lib.rs` 中定义 `SkillsConfig` 结构体（enabled: bool 默认 true, extra_paths: Vec<String>），派生 Deserialize + Default
- [x] 1.2 在 `Config` 结构体中添加 `skills: SkillsConfig` 字段，使用 `#[serde(default)]` 注解
- [x] 1.3 在 `config.example.toml` 中添加 `[skills]` 配置节的注释示例

## 2. Skill 发现与解析（krew-core）

- [x] 2.1 在 `krew-core` 的 `Cargo.toml` 中添加 `serde_yaml` workspace 依赖（在根 Cargo.toml 的 workspace.dependencies 中声明）
- [x] 2.2 在 `krew-core/src/` 下创建 `skill/` 模块目录，包含 `mod.rs`、`discovery.rs`、`types.rs`
- [x] 2.3 在 `types.rs` 中定义 `SkillRecord` 结构体和 `SkillError` 枚举
- [x] 2.4 在 `discovery.rs` 中实现 `parse_skill_md(path: &Path) -> Result<SkillRecord, SkillError>` 函数，解析 YAML frontmatter + Markdown body
- [x] 2.5 在 `discovery.rs` 中实现 `discover_skills(cwd: &Path, extra_paths: &[PathBuf]) -> Vec<SkillRecord>` 函数，按优先级扫描目录
- [x] 2.6 实现名称冲突处理逻辑：先发现的优先，冲突时记录 warn 日志
- [x] 2.7 为 `parse_skill_md` 和 `discover_skills` 编写单元测试

## 3. Skill Catalog 注入（krew-core）

- [x] 3.1 在 `skill/mod.rs` 中实现 `build_skill_catalog(skills: &[SkillRecord]) -> String` 函数，生成 XML 格式 catalog
- [x] 3.2 修改 `krew-core/src/agent/mod.rs` 中的 `start_completion()` 方法，在构建 system prompt 时注入 skill catalog
- [x] 3.3 修改 `build_system_prompt()` 函数签名，添加 `skill_catalog: Option<&str>` 参数，按 project-instructions → skill-catalog → agent-prompt 顺序组装
- [x] 3.4 为新的 system prompt 构建逻辑编写单元测试

## 4. activate_skill 工具（krew-tools）

- [x] 4.1 在 `krew-tools/src/builtin/` 下创建 `activate_skill.rs`，定义 `ActivateSkillTool` 结构体
- [x] 4.2 实现 `ActivateSkillTool::spec()` 方法，返回工具的 JSON Schema（参数 name: String）
- [x] 4.3 实现 `ToolHandler` trait：读取 SKILL.md、剥离 frontmatter、包装 XML 标签、枚举资源文件
- [x] 4.4 实现激活去重逻辑：跟踪已激活 skills，重复激活时返回提示信息
- [x] 4.5 修改 `builtin/mod.rs` 的 `create_full_registry()`，在有可用 skills 时注册 `activate_skill` 工具
- [x] 4.6 为 `ActivateSkillTool` 编写单元测试

## 5. Agent 初始化集成（krew-core）

- [x] 5.1 修改 `krew-core/src/agent/init.rs` 的 `init_agents()` 函数，在启动时调用 `discover_skills()` 并存储结果
- [x] 5.2 将 skill catalog 传递到 `AgentRuntime`，使其在 `start_completion()` 时可用
- [x] 5.3 将 `SkillRecord` 列表传递到 `ActivateSkillTool`，使其在执行时可以查找和读取 skills
- [x] 5.4 确保 `/new` 命令重置已激活 skills 的跟踪状态

## 6. /skills 命令实现（krew-cli）

- [x] 6.1 修改 `krew-cli/src/app/commands.rs` 中 `SlashCommand::Skills` 的处理分支，显示可用 skills 列表
- [x] 6.2 显示格式：每个 skill 的 name（高亮）、description、来源路径；无 skills 时显示 "No skills available"

## 7. 文档与示例

- [x] 7.1 更新 `config.example.toml` 添加 `[skills]` 配置节说明
- [ ] 7.2 在 `.krew/skills/` 下创建一个示例 skill（如 `code-review/SKILL.md`）用于测试
- [ ] 7.3 更新 PDD/TDD 文档，将 Agent Skills 从 v0.2 计划移到已实现功能

## 8. 集成测试与验证

- [x] 8.1 运行 `cargo fmt --all` 和 `cargo clippy --all-targets --all-features -- -D warnings` 确保代码规范
- [x] 8.2 运行 `cargo test` 确保所有测试通过
- [ ] 8.3 手动测试：放置一个 skill 到 `.krew/skills/`，启动 krew-cli，验证 `/skills` 显示、LLM 自动激活 skill 的完整流程
