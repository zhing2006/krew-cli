## 1. CLI 子命令注册

- [x] 1.1 在 `ConfigAction` 枚举中新增 `Help` 变体，在 clap 子命令中注册 `help`
- [x] 1.2 在 `config_cmd/mod.rs` 的 `dispatch()` 中添加 `Help` 分支，分发到 `help::run()`

## 2. 配置手册内容

- [x] 2.1 新建 `config_cmd/help.rs` 模块，实现 `run()` 函数，打印完整的硬编码英文配置手册
- [x] 2.2 手册包含文件位置和 merge 规则：user config 路径、project config 路径、各层支持的 section（user 不含 agents/reply_order）、merge 语义
- [x] 2.3 手册包含 `[settings]` 完整字段说明，标注 reply_order 仅 project config，所有默认值与代码常量一致
- [x] 2.4 手册包含 `[settings.retry]` 完整字段说明（含所有默认值）
- [x] 2.5 手册包含 `[providers.<name>]` 完整字段说明（type、api_key、api_key_env、base_url、vertex_project、vertex_location）
- [x] 2.6 手册包含 `[[agents]]` 完整字段说明（含 sampling 子表、默认值与代码一致），标注仅 project config
- [x] 2.7 手册包含 `[[mcp_servers]]` 完整字段说明（stdio 和 HTTP 两种传输模式）
- [x] 2.8 手册包含 `[skills]` 完整字段说明（enabled、extra_paths、默认值）
- [x] 2.9 手册包含示例配置片段（user config 和 project config 各一个）
- [x] 2.10 手册包含 CLI 命令参考（init/add/del/list/doctor/help 简要说明）

## 3. 系统提示词增强

- [x] 3.1 修改 `build_identity_prompt()` 在 "hosted by krew-cli" 之后加入 krew-cli 简介（一句话）
- [x] 3.2 修改 `build_identity_prompt()` 加入配置帮助提示（"执行 `krew config help` 获取配置手册"）

## 4. 清理过时 Azure 文档引用

- [x] 4.1 清理 `docs/MANUAL_CN.md` 中过时的 Azure 配置示例块
- [x] 4.2 清理 `docs/MANUAL.md` 中过时的 Azure 配置示例块
- [x] 4.3 清理 `docs/TDD.md` 中过时的 Azure 模式描述和 azure_endpoint/azure_api_version 字段

## 5. 测试

- [x] 5.1 集成测试：`krew config help` 输出包含所有章节标题
- [x] 5.2 集成测试：`krew config help` 输出包含所有 `[settings]` 字段名
- [x] 5.3 集成测试：`krew config help` 输出包含关键默认值（compact_keep_rounds=3、tools=true、worker_threads=4、agent_to_agent_max_rounds=10 等），直接引用代码常量或硬编码与代码一致的值
- [x] 5.4 集成测试：`krew config help` 输出包含所有 `[[agents]]` 字段名
- [x] 5.5 集成测试：`krew config help` 输出包含所有 `[providers.<name>]` 字段名
- [x] 5.6 集成测试：`krew config help` 输出包含所有 `[[mcp_servers]]` 字段名
- [x] 5.7 集成测试：`krew config help` 输出包含 `[skills]` 和 `[settings.retry]` 字段名
- [x] 5.8 单元测试：`build_identity_prompt()` 输出包含 krew 简介和配置帮助提示
- [x] 5.9 运行 `cargo fmt --all` 和 `cargo clippy`，确保通过
