## 1. krew-config 配置层

- [x] 1.1 在 `Settings` 结构体中添加 `language: Option<String>` 字段（默认 `None`）
- [x] 1.2 在 `RawSettings` 和 `UserSettings` 中添加 `language: Option<String>` 字段
- [x] 1.3 在 `RawConfig::merge_user` 中添加 `language` 的合并逻辑
- [x] 1.4 在 `RawConfig::resolve` 中添加 `language` 的解析逻辑（直接透传 `Option`）

## 2. krew-core 运行时层

- [x] 2.1 在 `AgentRuntime` 结构体中添加 `language: Option<String>` 字段
- [x] 2.2 在 `init_agents` 中从 `Settings.language` 初始化 `AgentRuntime.language`
- [x] 2.3 在 `start_completion` 的基础 identity 块中（日期时间行之后、peer/whisper 之前），当 `language` 为 `Some` 时注入语言指令

## 3. 配置示例

- [x] 3.1 在 `config.example.toml` 中添加 `language` 字段的注释说明和示例

## 4. 测试

- [x] 4.1 为 `build_language_instruction` 添加测试：验证设置 `language` 时返回正确的注入文本
- [x] 4.2 为 `build_language_instruction` 添加测试：验证未设置 `language` 时返回空字符串
- [x] 4.3 为 krew-config 添加测试：验证 language 的 resolve 默认值、值保留、project 覆盖 user、user 继承（4 个用例）

## 5. 验证

- [x] 5.1 运行 `cargo check --all-targets` 确认编译通过
- [x] 5.2 运行 `cargo clippy` 和 `cargo fmt` 确认无警告
