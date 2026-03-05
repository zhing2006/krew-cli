## Why

Phase 12 (交互打磨) 的补全、思考过程、ESC 中断等功能已完成，剩余两项核心能力尚未实现：Provider 原生 Web Search 和 Fetch URL 工具。Web Search 让 Agent 能获取实时信息，Fetch 让 Agent 能主动抓取指定网页内容，两者互补，是 Agent 实用性的关键补全。

## What Changes

- **Web Search 注入**：当 `enable_web_search = true` 时，在 LLM 请求的 tools 字段中注入 Provider 原生搜索工具（OpenAI Responses / Anthropic / Gemini），模型自主决定是否触发搜索
- **Chat API / Compatible 静默跳过**：OpenAI Chat Completions 和 Compatible provider 不支持 tool 注入式搜索，静默忽略 `enable_web_search` 配置
- **fetch_url 内置工具**：新增 built-in tool，接受 `url` 参数，抓取网页并通过 `htmd` crate 转换为 Markdown 返回
- **Fetch 审批机制**：默认需要用户审批，`settings.toml` 新增 `fetch_allow_domains` 白名单配置，白名单内域名免审批
- **Fetch 安全限制**：响应体最大 1MB，HTTP 自动升级 HTTPS，Follow redirects，合理 User-Agent

## Capabilities

### New Capabilities

- `web-search`: Provider 原生 Web Search 注入，按 provider 类型条件注入搜索工具到 API 请求
- `fetch-url`: fetch_url 内置工具，抓取网页内容并转换为 Markdown

### Modified Capabilities

（无现有 spec 需要修改）

## Impact

- **krew-llm**：各 provider 实现文件需增加 web search tool 注入逻辑，`LlmClientConfig` 新增 `enable_web_search` 字段
- **krew-tools**：新增 `fetch_url.rs` built-in tool，新增 `htmd` 依赖
- **krew-config**：新增 `fetch_allow_domains` 配置字段
- **krew-core**：传递 `enable_web_search` 和 `fetch_allow_domains` 配置到对应模块
- **依赖**：新增 `htmd` crate（HTML→Markdown 转换）
- **配置文件**：`config.example.toml`、`dev_plan.md` 需更新
