## 1. Web Search — 配置传递

- [x] 1.1 在 `LlmClientConfig` 中添加 `enable_web_search: bool` 字段
- [x] 1.2 在 krew-core 构建 `LlmClientConfig` 时从 `AgentConfig` 传递 `enable_web_search`

## 2. Web Search — Provider 注入

- [x] 2.1 OpenAI Responses: 当 `enable_web_search = true` 时在 tools 数组中追加 `{ "type": "web_search" }`
- [x] 2.2 Anthropic: 当 `enable_web_search = true` 时在 tools 数组中追加 `{ "type": "web_search_20250305", "name": "web_search" }`
- [x] 2.3 Google Gemini: 当 `enable_web_search = true` 时在 tools 数组中追加 `{ "google_search": {} }`
- [x] 2.4 OpenAI Chat / Compatible: 确认静默忽略 `enable_web_search`（无需改动，仅验证）

## 3. Fetch 工具 — 依赖与配置

- [x] 3.1 在根 `Cargo.toml` workspace dependencies 中添加 `htmd`
- [x] 3.2 在 `krew-tools/Cargo.toml` 中添加 `htmd` 和 `reqwest` 依赖
- [x] 3.3 在 `krew-config` 中添加 `fetch_allow_domains: Vec<String>` 字段（默认空数组）

## 4. Fetch 工具 — 实现

- [x] 4.1 创建 `krew-tools/src/builtin/fetch_url.rs`，实现 ToolHandler trait
- [x] 4.2 在 `krew-tools/src/builtin/mod.rs` 中注册 fetch_url 工具
- [x] 4.3 实现域名白名单审批逻辑（从 URL 提取 host，匹配 `fetch_allow_domains`）
- [x] 4.4 在 krew-core 中传递 `fetch_allow_domains` 配置到工具注册

## 5. 测试

- [x] 5.1 Web Search: 各 provider 请求构建测试（验证 tools 数组包含正确的搜索工具定义）
- [x] 5.2 Fetch: URL 验证测试（无效 URL、HTTP 升级 HTTPS）
- [x] 5.3 Fetch: 域名白名单匹配测试（精确匹配、子域名匹配、不匹配）
- [x] 5.4 Fetch: HTML→Markdown 转换测试
- [x] 5.5 Fetch: 响应体大小限制测试

## 6. 收尾

- [x] 6.1 更新 `config.example.toml` 添加 `fetch_allow_domains` 示例和 `enable_web_search` 注释
- [x] 6.2 更新 `docs/dev_plan.md` Phase 12 状态
- [x] 6.3 运行 `cargo fmt --all` 和 `cargo clippy` 确保通过
