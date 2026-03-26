## 1. Config 层 — Settings 结构体与默认值

- [x] 1.1 在 `krew-config/src/lib.rs` 的 `Settings` 结构体中添加 `restrict_workspace: bool` 字段（`#[serde(default = "default_true")]`）
- [x] 1.2 在 `krew-config/src/defaults.rs` 的 `Config::default()` 中设置 `restrict_workspace: true`

## 2. Config 层 — 分层配置链路（raw.rs）

- [x] 2.1 在 `krew-config/src/raw.rs` 的 `RawSettings` 结构体中添加 `restrict_workspace: Option<bool>` 字段
- [x] 2.2 在 `krew-config/src/raw.rs` 的 `UserSettings` 结构体中添加 `restrict_workspace: Option<bool>` 字段
- [x] 2.3 在 `RawConfig::merge_user()` 的 `merge_option!` 宏调用列表中添加 `merge_option!(restrict_workspace)`
- [x] 2.4 在 `RawConfig::resolve()` 中添加 `restrict_workspace: self.settings.restrict_workspace.unwrap_or(true)` 到 Settings 构造

## 3. 工具核心 — validate_path 与 5 个文件工具

- [x] 3.1 修改 `krew-tools/src/lib.rs` 中 `validate_path` 签名，增加 `restrict: bool` 参数；当 `restrict = false` 时跳过 `starts_with(cwd_canonical)` 检查
- [x] 3.2 修改 `krew-tools/src/builtin/read_file.rs`：ReadFileTool 构造函数增加 `restrict_workspace` 参数，调用 `validate_path` 时传入
- [x] 3.3 修改 `krew-tools/src/builtin/glob.rs`：GlobTool 构造函数增加 `restrict_workspace` 参数，`validate_path` 调用和遍历时的 `cwd_canonical` 前缀过滤都根据该标志决定是否执行
- [x] 3.4 修改 `krew-tools/src/builtin/grep.rs`：GrepTool 构造函数增加 `restrict_workspace` 参数，调用 `validate_path` 时传入
- [x] 3.5 修改 `krew-tools/src/builtin/edit_file.rs`：EditFileTool 构造函数增加 `restrict_workspace` 参数，调用 `validate_path` 时传入
- [x] 3.6 修改 `krew-tools/src/builtin/write_file.rs`：WriteFileTool 构造函数增加 `restrict_workspace` 参数，内联边界检查根据该标志决定是否执行

## 4. 注册层 — 工厂函数与调用方

- [x] 4.1 修改 `krew-tools/src/builtin/mod.rs` 中 `create_readonly_registry` 和 `create_full_registry` 签名，增加 `restrict_workspace: bool` 参数，传递给各工具构造函数
- [x] 4.2 修改 `krew-core/src/agent/init.rs`，从 `config.settings.restrict_workspace` 取值传入 `create_full_registry`
- [x] 4.3 修改 `krew-core/src/agent/approval.rs` 中 `create_full_registry` 调用处，传入 `restrict_workspace` 参数

## 5. 测试

- [x] 5.1 更新 `krew-tools/tests/` 下现有测试，适配 `validate_path` 和工具构造函数新签名
- [x] 5.2 为 `validate_path` 添加 `restrict = false` 时允许 workspace 外路径的测试
- [x] 5.3 为 write_file 添加 `restrict_workspace = false` 时允许 workspace 外路径的测试
- [x] 5.4 为 `RawConfig` 添加 `restrict_workspace` 合并与 resolve 的测试：验证 project 优先、user 补充、双方未设置三种场景
- [x] 5.5 运行 `cargo test` 确认全部通过
- [x] 5.6 运行 `cargo clippy --all-targets --all-features -- -D warnings` 确认无警告
- [x] 5.7 运行 `cargo fmt --all -- --check` 确认格式正确

## 6. 配置与帮助文本

- [x] 6.1 在 `config.example.toml` 的 `[settings]` 中添加 `restrict_workspace` 注释说明
- [x] 6.2 在 `krew-cli/src/config_cmd/help.rs` 的 MANUAL 常量中添加 `restrict_workspace` 条目（含类型、默认值、功能说明）

## 7. 文档更新

- [x] 7.1 更新 `docs/README.md`（EN）和 `docs/README_CN.md`（CN）：README 中无 settings 详细列表，跳过
- [x] 7.2 更新 `docs/MANUAL.md`（EN）和 `docs/MANUAL_CN.md`（CN）：添加 `restrict_workspace` 配置说明
- [x] 7.3 更新 `docs/PDD.md`：在配置示例中添加 `restrict_workspace`
- [x] 7.4 更新 `docs/TDD.md`：在 Settings 结构体文档中添加 `restrict_workspace` 字段说明
