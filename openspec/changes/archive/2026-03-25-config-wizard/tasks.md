## 1. 依赖与基础设施

- [x] 1.1 在 workspace `Cargo.toml` 中添加 `dialoguer` 和 `toml_edit` 依赖，在 `krew-cli/Cargo.toml` 中引入 `dialoguer`，在 `krew-config/Cargo.toml` 中引入 `toml_edit`
- [x] 1.2 在 `krew-cli/src/main.rs` 中扩展 clap 定义，添加 `config` 子命令组（init / add / del / list / doctor），保持无子命令时的 TUI 行为不变。所有 config 子命令统一使用 `tokio::runtime::Builder::new_current_thread()` 创建轻量 runtime
- [x] 1.3 在 `krew-cli/src/` 下创建 `config_cmd/` 模块目录，包含 `mod.rs`（子命令分发）、`init.rs`、`add.rs`、`del.rs`、`list.rs`、`doctor.rs`

## 2. krew-config 配置写入（config-file-writer）

- [x] 2.1 在 `krew-config` 中新建 `writer.rs` 模块，实现基于 `toml_edit::DocumentMut` 的文件读取/解析/写回基础函数（load_document、save_document），处理文件不存在时创建、父目录创建
- [x] 2.2 实现 `add_provider()` 函数：向 `[providers.<name>]` 表追加供应商配置，支持 type、api_key、api_key_env、base_url、vertex_project、vertex_location 字段
- [x] 2.3 实现 `remove_provider()` 函数：从配置文件中移除指定 `[providers.<name>]` 表
- [x] 2.4 实现 `add_agent()` 函数：向 `[[agents]]` 追加 Agent 表项，同步更新 `reply_order`
- [x] 2.5 实现 `remove_agent()` 函数：移除指定 `[[agents]]` 表项，同步更新 `reply_order`
- [x] 2.6 实现 `batch_add_agents()` 函数：批量写入多个 Agent 和完整 reply_order（仅 init 预设场景）。文件中已有 `[[agents]]` 时 SHALL 返回错误拒绝覆盖
- [x] 2.7 实现 `list_providers()` 和 `list_agents()` 读取函数
- [x] 2.8 为 writer 模块编写集成测试：add/remove/batch_add 的各场景，格式保留验证，错误场景（包括 batch_add 拒绝覆盖）

## 3. krew-llm List Models API（llm-list-models）

- [x] 3.1 在 `krew-llm` 中新建 `list_models.rs` 模块，定义 `ModelInfo` 结构体、`ListModelsConfig` 结构体（含 provider_type、base_url、api_key、vertex_project、vertex_location）和 `list_models()` 异步函数签名
- [x] 3.2 实现 OpenAI List Models：`GET /v1/models`，Bearer 认证，过滤 gpt/o/chatgpt 开头的模型
- [x] 3.3 实现 Anthropic List Models：`GET /v1/models`，X-Api-Key + anthropic-version 认证，过滤 claude- 开头的模型
- [x] 3.4 实现 Google Gemini API List Models：`GET /v1beta/models?key=&pageSize=1000`，去掉 `models/` 前缀，过滤 gemini- 开头的模型
- [x] 3.5 实现 Google Vertex AI List Models：`GET https://{location}-aiplatform.googleapis.com/v1/projects/{project}/locations/{location}/publishers/google/models`，Bearer 认证，提取 `publishers/google/models/` 前缀后的 id，过滤 gemini- 开头的模型
- [x] 3.6 实现 5 秒超时和 `fallback_models()` 硬编码列表（Anthropic: claude-opus-4-6/sonnet-4-6/haiku-4-5，OpenAI: gpt-5.4/5.4-mini/5.4-nano，Google: gemini-3.1-pro-preview/flash-lite-preview）
- [x] 3.7 实现模型列表按 id 字母排序
- [x] 3.8 为 list_models 编写单元测试（mock HTTP 响应、过滤逻辑、fallback、超时、Vertex AI 端点构造和 id 提取）

## 4. CLI 交互流程（config-wizard-init）

- [x] 4.1 实现 `init.rs` 中的智能分流逻辑：检测 user/project 配置文件存在状态，决定进入 User Init / Project Init / 提示已存在。`--user`/`--project` 标志遵循 bootstrap 语义：对应文件已存在时拒绝并引导到 add/del
- [x] 4.2 实现 User Init 供应商配置循环：Select 类型 → Input 名称 → Select key 存储方式 → 收集 key → 可选 base_url/vertex → Confirm 继续 → 循环结束后摘要表格。所有提示文本使用英文
- [x] 4.3 实现 Project Init 创建方式选择：读取 merge 后的配置（user + project）获取可用供应商列表，Select "Smart Preset" / "Manual Setup"
- [x] 4.4 实现 Smart Preset 流程：获取所有供应商模型 → 根据候选数决定预设选项 → "Single Agent" / "Three Agents" 选择 → 属性自动推导（enable_thinking 默认 true，通过 Confirm 确认）→ 确认写入
- [x] 4.5 实现手动逐个创建 Agent 循环：Select 供应商 → Select/FuzzySelect 模型 → Input 名称/显示名 → Select 颜色 → Confirm thinking/web_search → Confirm 继续
- [x] 4.6 实现 User Init 完成后衔接 Project Init 的 Confirm 提示

## 5. CRUD 命令（config-wizard-crud）

- [x] 5.1 实现 `add.rs`：add provider 交互流程（复用 init 中的单个供应商添加逻辑），add agent 交互流程（复用手动创建单个 Agent 的逻辑，供应商列表从 merge 后配置获取）
- [x] 5.2 实现 `del.rs`：del provider（Select 选择 + 引用检查警告 + Confirm），del agent（Select 选择 + reply_order 同步 + 最后一个 Agent 警告）。所有提示文本使用英文
- [x] 5.3 实现 `list.rs`：list providers 表格输出（Name/Type/Key Method/Base URL + 环境变量状态检测），list agents 表格输出（Name/Display/Provider/Model/Color/Thinking/Web + reply_order 展示）

## 6. Doctor 诊断命令（config-wizard-doctor）

- [x] 6.1 实现 `doctor.rs`：自行读取并解析配置文件（不使用 `UserConfig::load()` 的静默 fallback），TOML 解析失败时明确报告 parse error。解析成功后使用与运行时相同的 merge 逻辑获取最终配置，进行供应商 key 可用性检查、Agent 供应商引用检查。所有输出使用英文
- [x] 6.4 为 doctor 编写测试：覆盖 user/project 配置文件 TOML 损坏场景（报告 parse error 而非静默回退为默认值），以及正常诊断场景
- [x] 6.2 实现 MCP 服务器诊断：stdio 类型检查 command 可用性，HTTP 类型展示 url
- [x] 6.3 实现诊断总结：汇总通过/失败数，输出最终状态

## 7. 集成测试与验证

- [x] 7.1 为 CLI 子命令解析编写测试：验证 clap 正确解析所有 `krew config *` 子命令和标志，包括 --user/--project 互斥
- [x] 7.2 端到端测试：模拟 init 流程生成的配置文件能被 `Config::load()` 正确加载和验证
- [x] 7.3 `cargo fmt --all` 和 `cargo clippy --all-targets --all-features -- -D warnings` 通过
