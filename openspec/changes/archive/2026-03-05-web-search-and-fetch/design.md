## Context

krew-cli 的 Agent 目前只能通过内置工具（read_file、shell 等）和 MCP 工具与外部世界交互。缺少两个关键能力：利用 Provider 原生 Web Search 获取实时信息，以及主动抓取指定 URL 的网页内容。

当前状态：
- `enable_web_search: bool` 配置字段已存在于 `AgentConfig`，默认 false，但未接入任何实现
- `LlmClientConfig` 尚未包含该字段，各 provider 实现未处理搜索工具注入
- 内置工具系统已成熟（ToolSpec/ToolHandler/ToolRegistry），新增工具有清晰的模式可循
- 审批机制已有 `shell_allow_commands` 前缀匹配模式可参考

## Goals / Non-Goals

**Goals:**
- 将 `enable_web_search` 配置传递到各 LLM provider，在 API 请求中注入原生搜索工具
- 新增 `fetch_url` built-in tool，支持抓取网页并转换为 Markdown
- 提供 `fetch_allow_domains` 白名单机制控制审批

**Non-Goals:**
- 不实现引用脚注 `[n]` 显示
- 不支持 OpenAI Chat Completions API 的 web search（该 API 不支持 tool 注入式搜索）
- 不支持 Compatible provider 的 web search
- 不在 fetch_url 中引入小模型做内容预处理（直接返回转换后的 Markdown）
- 不实现 fetch 结果缓存

## Decisions

### 1. Web Search 注入位置：在 request body 构建时注入

在各 provider 的 `chat_stream()` 方法中，构建请求 body 时根据 `enable_web_search` 字段条件注入。搜索工具与用户自定义工具共存于同一 tools 数组。

**替代方案**：在 ToolRegistry 层面注入 → 不可行，因为 web search 是 provider 原生能力，不走我们的 Tool dispatch 系统。

### 2. 各 Provider 注入格式

- **OpenAI Responses**: `{ "type": "web_search" }` 追加到 tools 数组
- **Anthropic**: `{ "type": "web_search_20250305", "name": "web_search" }` 追加到 tools 数组
- **Google Gemini**: `{ "google_search": {} }` 追加到 tools 数组
- **OpenAI Chat / Compatible**: 静默忽略，不注入不报错

### 3. HTML→Markdown 转换使用 htmd crate

选择 `htmd` 而非 `html2md` 或 `fast_html2md`：
- API 干净（turndown.js 风格）
- 依赖轻量（html5ever + markup5ever_rcdom + phf）
- 0.5.0 版本，成熟度适中
- 纯 Rust 依赖，不引入 C 库（`html2md` 默认用 `lol_html`）
- 与当前依赖树无重叠，不会 dup

### 4. fetch_url 审批模式：复用 shell 审批的域名匹配模式

`fetch_allow_domains` 在 `settings.toml` 中与 `shell_allow_commands` 同级，域名前缀匹配：
- `github.com` 匹配 `github.com` 及所有子路径
- `docs.rs` 匹配 `docs.rs/...`

域名提取自 URL 的 host 部分，匹配逻辑：白名单中的域名是目标 URL host 的后缀（支持子域名匹配）。

### 5. fetch_url 安全约束

- 响应体最大 1MB，超过则截断并提示
- HTTP 自动升级 HTTPS
- Follow redirects（reqwest 默认行为）
- User-Agent: `krew-cli/0.1.0`
- 超时 30 秒

## Risks / Trade-offs

- **[Web Search token 消耗]** → 搜索结果会增加 input tokens，但这是 provider 原生行为，用户通过 `enable_web_search` 显式开启
- **[Anthropic web_search 版本号]** → 使用 `web_search_20250305` 硬编码版本号，未来可能需要更新 → 可通过配置字段扩展
- **[htmd 转换质量]** → HTML→Markdown 转换可能丢失部分结构信息 → 对 LLM 使用场景已足够
- **[fetch_url 不支持 JS 渲染页面]** → SPA 页面抓取内容可能为空 → 这是合理限制，与 Claude Code 行为一致
