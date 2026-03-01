## 1. ConfigError 和基础设施

- [x] 1.1 在 `krew-config/src/lib.rs` 中定义 `ConfigError` 枚举（Io、Parse、Validation），使用 thiserror 派生。确认 toml crate 启用 `serde` feature 以支持 `#[from] toml::de::Error`
- [x] 1.2 在 `krew-config/src/lib.rs` 中添加 `CONFIG_FILENAME` 公开常量，值为 `".krew/settings.toml"`

## 2. Config 默认值

- [x] 2.1 在 `krew-config/src/defaults.rs` 中为 `Config` 实现 `Default` trait，返回包含单个 echo agent 的最小配置（详见 config-loading spec）
- [x] 2.2 为 `Settings`、`AgentConfig` 等类型补充必要的 derive（如 `Clone`），以支持 Default 实现

## 3. Config 加载

- [x] 3.1 在 `krew-config/src/lib.rs` 中实现 `Config::load(path: &Path) -> Result<Config, ConfigError>`，读取文件内容并调用 `toml::from_str` 反序列化
- [x] 3.2 编写 `Config::load()` 单元测试：有效文件加载、文件不存在、格式错误

## 4. Config 校验

- [x] 4.1 在 `krew-config/src/lib.rs` 中实现 `Config::validate(&self) -> Result<(), ConfigError>`，检查 reply_order 引用、provider 引用、agent name 唯一性，builtin provider 跳过检查
- [x] 4.2 编写 `Config::validate()` 单元测试：有效配置、无效 reply_order 引用、无效 provider 引用、重复 agent name、builtin provider 跳过

## 5. CLI 覆盖

- [x] 5.1 在 `krew-config/src/lib.rs` 中为 `ApprovalMode` 添加 `from_str` 解析方法（或实现 `FromStr`）
- [x] 5.2 在 `krew-config/src/lib.rs` 中实现 `Config::apply_cli_overrides(&mut self, agents: Option<&str>, approval_mode: Option<&str>) -> Result<(), ConfigError>`
- [x] 5.3 编写 `apply_cli_overrides()` 单元测试：无覆盖、--agents 过滤、--agents 无效名称、--approval-mode 覆盖、无效模式

## 6. krew-cli 启动流程集成

- [x] 6.1 修改 `krew-cli/src/main.rs` 的 `main()` 函数：在日志初始化后加载配置文件（--config 路径或默认路径），文件不存在时使用 Config::default()，然后 apply_cli_overrides 和 validate
- [x] 6.2 修改 `App::new()` 签名，接收 `Config` 参数并存储在 App 结构体中
- [x] 6.3 添加配置加载失败时的用户友好错误提示（格式错误含行号、引用不合法含名称）

## 7. 启动 banner 和状态栏

- [x] 7.1 在 `krew-cli/src/render.rs` 中添加颜色字符串到 `ratatui::style::Color` 的映射函数
- [x] 7.2 修改 `insert_header()` 显示 Agent 列表行，按 reply_order 顺序，[name] 使用 agent 颜色着色
- [x] 7.3 修改 `render_status_bar()` 从 `App.config` 读取 approval_mode 和 auto_compact_threshold 的实际值

## 8. 示例配置和收尾

- [x] 8.1 在项目根目录创建 `config.example.toml`，包含完整的配置示例（基于 PDD §4.6.2）
- [x] 8.2 运行 `cargo fmt --all` 和 `cargo clippy --all-targets --all-features -- -D warnings` 确保代码质量
- [x] 8.3 运行 `cargo test` 确保所有测试通过
