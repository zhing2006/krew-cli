## 1. Config 层新增字段

- [x] 1.1 在 `Settings` 结构体新增 `update_check: bool` 字段（默认 `true`）
- [x] 1.2 在 `RawSettings` 和 `UserSettings` 新增 `update_check: Option<bool>` 字段，更新合并逻辑
- [x] 1.3 更新 `config.example.toml` 添加 `update_check` 示例

## 2. 核心版本检查模块

- [x] 2.1 在 `krew-cli/src/` 新建 `update_check.rs` 模块，包含缓存结构体定义（`VersionCache`：`latest_version` + `checked_at`）
- [x] 2.2 实现缓存读取函数：读 `~/.krew/version_check.toml`，反序列化失败返回 None
- [x] 2.3 实现缓存写入函数：序列化写入 `~/.krew/version_check.toml`，失败静默忽略
- [x] 2.4 实现 npm registry 请求函数：GET `https://registry.npmjs.org/@zhing2026/krew/latest`，2 秒超时，提取 `version` 字段；失败/超时时写入当前版本作为缓存（24h 冷却）
- [x] 2.5 实现版本比较函数 `compare_versions(current, latest) -> Option<Ordering>`：按 `.` 分割，逐段解析 `u32`，解析失败返回 None，段数不足补 0
- [x] 2.6 实现异步主入口函数 `async fn check_for_update(enabled: bool) -> Option<String>`：整合缓存检查 → npm 请求 → 版本比较 → 返回警告消息

## 3. 启动流程集成

- [x] 3.1 `krew-cli` 新增 `reqwest` workspace 依赖；在 `main.rs` 中 tokio runtime 建立后、prompt/TUI 分支前，通过 `runtime.block_on(check_for_update(...))` 调用，将返回的警告 push 到共享 warnings 列表
- [x] 3.2 统一 warnings 分发：prompt 模式输出到 stderr（与 config_warnings 合并打印），TUI 模式灌入 `app.startup_warnings`

## 4. 文档与帮助文本更新

- [x] 4.1 更新 `config help` 命令的硬编码文本，添加 `update_check` 字段说明
- [x] 4.2 更新 `docs/MANUAL.md` 添加 `update_check` 配置项说明
- [x] 4.3 更新 `docs/MANUAL_CN.md` 添加 `update_check` 配置项说明

## 5. 测试

- [x] 5.1 为 `compare_versions` 函数编写单元测试（覆盖：落后、相同、超前、MAJOR 落后但后续段更大、段数不同、解析失败等场景）
- [x] 5.2 更新 config merge 测试（`raw_config_test.rs`），验证 `update_check` 字段的分层合并逻辑
- [x] 5.3 更新 config help 测试（`config_help_test.rs`），验证输出包含 `update_check` 字段说明
