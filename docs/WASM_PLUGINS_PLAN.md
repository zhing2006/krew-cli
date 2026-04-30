# krew-cli → WASM 插件化重构计划

> 分支: `arch/wasm-plugins`
> 目标版本: v1.0.0（Breaking change，重构期 v0.x 继续在 main 上做 bugfix）
> 创建日期: 2026-04-30

---

## 0. TL;DR

把 krew-cli 拆为「**最小宿主 (host bin) + 一组 WASM 组件 (guest .wasm)**」两层架构：

- **Host (krew-cli bin)** —— 仍是原生 Rust 二进制（x86_64 / aarch64，**绝不降级到 32 位**），保留 TUI、Agent Loop 调度、配置/会话 IO、基础文件工具，并嵌入 wasmtime 加载和驱动插件。
- **Guest (.wasm 插件)** —— 编译目标**统一定为 `wasm32-wasip3`**（WASI 0.3 / Component Model，**preview 阶段，未正式稳定**——Rust 标 Tier 3、Wasmtime 37+ 标 experimental，详见 §3.1）。承载所有"高级功能"：`@`/`#` 路由、多 Agent 编排、LLM Provider、Skill、MCP、Memory、CLAUDE/AGENTS.md、`web_search` 等带网络的高级 tools。

通信走 **WIT (`wit-bindgen`) + Component Model**，HTTP/SSE 复用标准 `wasi:http@0.3` 接口，TLS 终止留 host。这是一项明确的前瞻性押注（不走 wasip2 过渡）——决策与代价见 §3.1。

---

## 1. 目标 / 非目标

### 1.1 目标
1. 让"协议层 / 功能层"可独立迭代、独立分发，不再每次加 Provider 都要发 krew-cli 新版本。
2. 第三方可以编写 .wasm 插件扩展 krew-cli（Provider、Skill 实现、命令、MCP 适配器等），无需 fork。
3. 插件崩溃不连累 host；插件之间通过受控 ABI 隔离，不能直接读对方内存。
4. 保留当前 v0.11 用户体验（TUI、`@`/`#`、@all 串行、ESC 取消、pending queue 等），不做语义回退。
5. 单文件分发不变（host 静态链接，常用插件可内嵌为 `include_bytes!`，外部插件可放 `.krew/plugins/*.wasm`）。

### 1.2 非目标
1. 不做"插件市场 / 远程下载"。v1.0 只支持本地 .wasm 文件。
2. 不做跨语言插件 SDK（暂不发 Go/Python PDK），插件作者首选 Rust。Extism 路线放弃。
3. 不替换 TUI 层。ratatui + crossterm 仍是 host 唯一渲染栈。
4. 持久化格式（Session TOML、settings.toml、plugins.lock）一律按新 schema 重写，**不做向后兼容**（pre-GA，详见 §9.2）。旧 session 文件解析失败即报错，用户自行清理 `.krew/sessions/`。
5. 不做插件签名验证（v1.1 引入 ed25519 签名）。但 v1.0 必须有最小可用的可信链：内嵌插件默认可信，外部插件强制 hash lockfile + 显式 trust 流程，详见 §8。

---

## 2. 现状评估

当前代码体量（粗略）：

| 区域 | 大致 LOC | 适配度 |
|---|---|---|
| `krew-cli` (TUI、main、prompt_mode、custom_terminal、render、streaming) | ~12k | **留 host** |
| `krew-core::agent` (agent_loop、prepare、approval、prune、init) | ~3k | **拆分**：调度骨架留 host，prompt 组装 / 消息折叠搬 guest |
| `krew-core::router` (`@`/`#` 解析与路由) | ~600 | **全搬 guest** |
| `krew-core::skill / sub_agent / task / custom_command / memory / compact / dream` | ~3k | **全搬 guest** |
| `krew-core::persistence / process_stats / event / discovery` | ~800 | **留 host**（Session IO、ProcessStats）/ 部分搬 guest（discovery 文件遍历由 host 提供 fs，发现逻辑 guest 拥有） |
| `krew-llm` (5 Provider + types + common + list_models) | ~5k | **每个 Provider 一个 guest**（HTTP 走 `wasi:http`） |
| `krew-tools::builtin` (8 个内置工具) | ~2k | **拆**：read_file/write_file/edit_file/glob/grep/shell 留 host；fetch_url、activate_skill、未来的 web_search 搬 guest |
| `krew-tools::mcp` (rmcp 包装) | ~800 | **搬 guest**：MCP 协议帧解析在 guest，stdio/http 传输由 host 暴露 |
| `krew-storage` | ~500 | **留 host** |
| `krew-config` | ~1500 | **保留 host**：公共 schema（settings、agents、permission、reply-order、mcp-servers）解析与 `Config::validate()` 全留 host；只有 `[plugins.config.<id>]` 子表透传给目标 plugin，由 plugin 自己反序列化（详见 §9.3）。AGENTS.md / instructions 加载逻辑搬 guest（见 Wave 2.4） |

**结论**：约 60% 的 LOC 会迁出 host，host 收敛到 ~15k LOC 上下，主体是 TUI 和 wasmtime 嵌入层。

---

## 3. 技术栈选型（依据已完成的网络调研）

| 选项 | 选 / 不选 | 理由 |
|---|---|---|
| **wasmtime 37+** + Component Model + `wasi:p3` | ✅ | Wasmtime 37+ 提供 WASI 0.3 preview，native async / `stream<T>` / `future<T>` 成为一等类型；spec completion 预计 2026/02 前后，**目前仍属 preview** |
| **`wasmtime::component::bindgen!` 宿主绑定** | ✅ | 强类型、零 boilerplate；用 `imports/exports: { default: async }` |
| **`wit-bindgen` guest 绑定** | ✅ | 标准做法，配 `cargo build --target wasm32-wasip3` |
| **`wasi:http@0.3.0` 标准接口（host 提供 outgoing-handler）** | ✅ | 不自造 SSE 桥；host 端用 `wasmtime-wasi-http`（hyper + tokio）实现 |
| **TLS 终止位置：host** | ✅ | guest 下 ring/aws-lc 不可用，rustls-rustcrypto 尚不建议生产；统一由 host hyper-rustls 终止 |
| **wasmtime AOT 缓存（`Engine::precompile_component`）** | ✅ | 缓解冷启动，常用插件首启后落 `~/.krew/cache/aot/*.cwasm` |
| **Extism PDK** | ❌ | 弱类型、流式差，krew 是纯 Rust 项目用不上多语言矩阵 |
| **wasmer** | ❌ | 不跟 component model 主线，标准支持滞后 |
| **wasm64 / Memory64** | ❌ | Rust 端 Tier 3，WASI 主流目标仍是 wasm32；host 64 位即可 |
| **`tokio` in guest** | ❌ | 官方仅支持 WASI 0.1；guest 内用 `wstd` 或裸 async（WASI 0.3 + futures） |
| **`reqwest` in guest** | ❌ | 直接用 `wasi:http`，不要在 guest 塞 reqwest |

### 3.1 关键决策：直接押 wasip3，不做 wasip2 过渡

**这是一个明确的前瞻性押注。** 评估后接受以下事实并选定 wasip3：

**为什么不先 wasip2 过渡：**
- WASI 0.2 的 async 是 polled-future 模型（每个异步操作维护 pollable 句柄、靠 `poll-list` 等待），SSE/streaming 这种 hot path 要写大量手写状态机；后续升 p3 时，绑定层需要重写而非"无痛升级"（虽然 spec 声称 95% 兼容，实际 wit-bindgen 输出形态变化很大）。
- krew-cli v1.0.0 是一个 12-18 个月稳定承诺周期的版本，从 p2 升 p3 的成本会折算成两次重大重构；不如一步到位。
- `wasi:http@0.3` 的 incoming-body `stream<u8>` 直接对应 SSE 分片读，是该场景的最佳形态。

**接受的代价（必须承认）：**
| 代价 | 缓解 |
|---|---|
| `wasm32-wasip3` 在 Rust 仍是 Tier 3，部分情况要 nightly 或特定 stable 版本 | 仓库根加 `rust-toolchain.toml` 锁定快照（host 64 位 stable + guest wasip3 工具链一并固定） |
| Wasmtime 37 的 p3 标注 experimental / unstable | 锁定 wasmtime minor 版本；CI 跑全量等价测试，每次 wasmtime 升级都重跑 |
| 部分 guest 端 crate（hyper / rustls / mio / 大多 tokio 生态）p3 适配滞后 | 这正是把 TLS / HTTP / 子进程 IO 全留 host 的根本原因；guest 只走 `wasi:http@0.3`、`wasi:io/streams@0.3`，不引入完整 async runtime（用 `wstd` 或裸 futures） |
| 实验阶段可能踩 wasmtime/wit-bindgen bug | Wave 0 提前在最小路径上验证 toolchain 健康度（见 §6） |

**回退预案：** v1.0 RC 阶段如果 p3 出现 blocker（例如 wasmtime 严重回归 / 关键 crate 长期不适配），PR 一份 `wasi:http@0.2` 兼容 binding 层。改动主要集中在 `krew-wit` 入口和 host 的 linker 注册，预计 1-2 周；插件源码因为已经写成 component model 风格，绝大多数无需改动。**只在出问题时启用此预案，不预先实现双轨。**

### 3.2 Async 策略（Host / WIT / Guest 三层）

为什么要单独说：传统 Rust 项目的 async 策略基本就是"用 tokio"。WASM 插件化引入两道额外的 async 边界——**WIT component-model async** 和 **guest 端有限的 async runtime 选择**——必须分清楚谁是异步的、谁不是、async 在哪儿落到同步上。

| 层 | 是否 async | 选型 / 实现 |
|---|---|---|
| **Host 应用层（krew-cli / krew-core）** | **混合**：krew-core 保持纯 sync；krew-cli / krew-host 全 async | core 不引入 async 是为了不和 wasmtime 边界耦合（详见 Wave 0 步骤 0.5/0.6）。所有 await 都发生在 krew-cli 调用方，await 完拿到纯数据切片再喂给 sync core |
| **Host 运行时** | **全 async**（tokio 多线程） | wasmtime `Engine` 配 `Config::async_support(true)`；wasi-http 用 hyper + tokio；wasi-clocks/random/cli 用 wasmtime 默认 async 实现 |
| **WIT 接口层（component model async）** | **押 native async** | `wasmtime::component::bindgen!(async: { only_imports: [...] })` 或 `default: async`。所有 host 提供的 import（HTTP、time、log、session、bus）默认按 async fn 暴露给 guest；export 的具体 async 与否取决于 WIT 签名 |
| **WIT 函数签名约定** | **按需声明** | (1) 纯 CPU 计算 / 立即返回的 export（如 router 的 parse-input、prompt-contributor 的 contribute）保持非 async 签名——host bindgen 仍把它包成 async 调，但 guest 函数体可以直接 return；(2) 涉及 host import IO 或返回 stream/future 的 export（如 provider 的 chat-stream、mcp 的 invoke）用 async 签名 |
| **Guest 内部** | **轻量 / 按需** | 不引入完整 tokio 生态（wasip3 上 tokio 还没适配好）。需要 await host import 时用 `wstd` 的轻量 executor 或裸 `futures` 适配 WASI 0.3 awaitable。**guest 不应起线程池、不应运行长寿命任务**——所有"耗时操作"实际由 host import 异步执行 |

**举例对照：**

| 插件 | guest export 签名 | guest 内部 | 实际 IO 在哪 |
|---|---|---|---|
| `datetime`（Wave 0） | `contribute(name) -> contribution`（非 async） | 调 `host_time::now_local_hour_aligned()` 一次 | host 端 `chrono::Local::now()` |
| `router`（Wave 1） | `parse-input(text, agents) -> result`（非 async） | 纯字符串扫描 | 无 |
| `provider-openai-chat`（Wave 3） | `chat-stream(...) -> stream<provider-event>`（async） | 用 wstd 跑 await loop：发请求 → 读 SSE incoming-body → 解帧 → push 事件 | host 的 `wasi:http` + hyper |
| `mcp-client`（Wave 4） | `invoke(name, args) -> result`（async） | await host stdio handle 的 read/write | host 起的子进程 |

**绝对禁止：**
- 在 krew-core 的同步路径里 `block_on` 任何 wasmtime 调用。
- 在 guest 内 spawn 后台线程（WASI 当前不允许多线程；wasi-threads 仍 experimental）。
- 在 guest 内引入 tokio 的 `tokio::main` / `tokio::spawn`。

**测试时机：** Wave 0 用 datetime 验证"非 async export + async host import"链路；Wave 1 mock-provider 验证"async export + async host stream"链路。两条全跑通后，后续 Wave 不再需要重新论证 async 策略。

---

## 4. 架构总览

```txt
┌──────────────────────────────────────────────────────────────────────────┐
│                          krew-cli HOST  (native bin)                      │
│  ┌──────────────────┐  ┌──────────────────┐  ┌────────────────────────┐ │
│  │ TUI (ratatui)    │  │ Session IO       │  │ Wasmtime Engine        │ │
│  │ - viewport       │  │ - .krew/sessions │  │ - 1 Engine (shared)     │ │
│  │ - approval       │  │ - .krew/logs     │  │ - Store per (sess,plug) │ │
│  │ - streaming      │  │                  │  │ - AOT cache             │ │
│  └────────┬─────────┘  └────────┬─────────┘  └──────────┬─────────────┘ │
│           │                     │                       │               │
│  ┌────────▼─────────────────────▼───────────────────────▼─────────────┐ │
│  │                   Plugin Manager  (host orchestrator)              │ │
│  │   - 加载 .wasm  - 实例化  - 路由事件  - 仲裁谁调谁                  │ │
│  └────────┬───────────────────┬───────────────────┬────────────────────┘ │
│           │ host imports (WIT)│                   │ host exports (WIT)   │
│           ▼                   ▼                   ▼                      │
│   wasi:http  wasi:filesystem  wasi:cli  krew:host/{ui,session,bus,fs} │
└──────────────┬──────────────┬──────────────┬─────────────┬──────────────┘
               │              │              │             │
        ┌──────▼─────┐  ┌─────▼─────┐  ┌─────▼─────┐  ┌────▼──────┐
        │ router.wasm│  │provider.  │  │ skill.    │  │ mcp.wasm  │
        │ (@/# parse)│  │  openai.  │  │  wasm     │  │           │
        │            │  │  wasm     │  │           │  │           │
        └────────────┘  └───────────┘  └───────────┘  └───────────┘
        ┌────────────┐  ┌───────────┐  ┌───────────┐  ┌───────────┐
        │ memory.    │  │provider.  │  │ web_      │  │ compact.  │
        │  wasm      │  │ anthropic │  │ search.   │  │  wasm     │
        │            │  │  .wasm    │  │  wasm     │  │           │
        └────────────┘  └───────────┘  └───────────┘  └───────────┘
```

### 4.1 进程模型
- **单进程**：所有 .wasm 在 host 进程内由 wasmtime 加载，靠线性内存 + WIT 类型隔离，不开子进程。
- **Store 粒度 = 每 (session, plugin) 一个 Store**：每个 session 为它用到的每个 plugin 各开一个 wasmtime `Store<HostState>`。理由：fuel / memory limit / epoch interruption / trap 隔离都是 Store 维度的能力；如果一个 session 内多个 plugin 共用一个 Store，A 插件死循环会拖死 B。Engine 全局共享（编译产物 + AOT cache 复用）。
- **常驻 vs 一次性**：常驻型插件（router、memory、orchestrator）随 session 生命周期；一次性插件（compact、dream）按需实例化、用完即 `drop(Store)` 回收。
- **崩溃域**：单个 Store trap 只影响该 (session, plugin) 二元组，wasmtime 把错误返回给 host，由 PluginManager 决定：重试 / 标记失效 / 回退默认实现。

### 4.2 数据所有权
- **Session 消息历史 = host 唯一真相源**。Plugin 通过 `krew:host/session` 接口读取（增量订阅或一次性 snapshot），写回也走 host 仲裁，禁止 plugin 之间直接交换 ChatMessage。
- **配置 = host 加载 + 分发**。Host 解析 `settings.toml` 骨架，找到 plugin 引用后把对应的配置子树（任意 TOML 子表）作为字节序列传给目标 plugin，由 plugin 自己反序列化（这样 Provider 插件可以加自己专属字段，host 不必知道）。
- **UI 渲染权 = host 独占**。Plugin 不直接画屏，发出语义事件（`UiEvent::AgentTextDelta`、`ApprovalRequest`），host 端 TUI 决定怎么渲染。

---

## 5. WIT 接口草案（v0 雏形，最终版会单独立 spec）

包名 `krew:host` (host 提供) 与 `krew:plugin` (guest 实现)，共用 `krew:types`。

### 5.1 共享类型 `krew:types`

```wit
package krew:types@1.0.0;

interface messages {
  enum chat-role { system, user, assistant, tool }

  // 与 crates/krew-llm/src/lib.rs::ChatMessage 一一对应；新增字段必须同步
  record chat-message {
    role: chat-role,
    name: option<string>,                          // assistant -> agent name; tool -> tool name
    content: string,
    tool-calls: list<tool-call-info>,              // 空 list 等价于 Rust 端 Option::None
    tool-call-id: option<string>,
    server-tool-uses: list<server-tool-use-info>,  // provider-executed tools (web_search 等)
    addressee: option<string>,                     // user 消息: "all" | agent 名
    whisper-targets: option<list<string>>,         // 设置时仅组内可见
    usage: option<usage>,
    created-at: string,                            // ISO-8601 UTC
    images: list<image-content>,                   // host 端不持久化（与 Rust #[serde(skip)] 等价）
  }

  record tool-call-info {
    id: string,
    name: string,
    arguments: string,                             // JSON string
    thought-signature: option<string>,             // Google thinking 签名（必须原样回传）
  }

  record server-tool-use-info {
    name: string,
    query: option<string>,                         // 例如 web_search 的 query
  }

  record image-content {
    data: list<u8>,
    media-type: string,                            // 例如 "image/png"
    filename: option<string>,
  }

  // ---- WIT ↔ Rust 类型映射规则（统一约定） ----
  // 1. WIT `list<T>` 对应 Rust `Vec<T>`，原样映射。
  // 2. 当 Rust 端实际类型是 `Option<Vec<T>>` 时，WIT 用 `list<T>`，
  //    **空 list 等价于 None**，host 序列化器负责双向 normalize。
  //    截至 v1.0 实际影响字段只有 chat-message.tool-calls 一个；
  //    chat-message.server-tool-uses 与 chat-message.images 在 Rust 端本就是 Vec，
  //    不走 normalize；chat-message.whisper-targets 必须保留 option<list<string>>
  //    （"未设置" = 非密语；"空集" 会被解释为非法），不能用空 list 替代。
  // 3. WIT `option<T>` 对应 Rust `Option<T>`，原样映射，不做 normalize。
  // 4. WIT `string` 对应 Rust `String`；空字符串就是空字符串，不与 None 互换。
  // ---------------------------------------------

  record usage {
    prompt-tokens: u32,
    completion-tokens: u32,
    total-tokens: u32,
  }
}

interface agents {
  record agent-snapshot {
    name: string,
    display-name: string,
    color: string,
    model: string,
    provider: string,                      // 引用 provider 插件 ID
  }
}
```

### 5.2 Host 提供的能力 `krew:host`

```wit
package krew:host@1.0.0;

interface ui {
  use krew:types/messages.{chat-message};

  // 流式 token 推送（agent text / thinking）
  resource ui-stream {
    push-text: func(delta: string);
    push-thinking: func(delta: string);
    finish: func(usage: krew:types/messages.usage);
    error: func(message: string);
  }

  // 工具事件
  notify-tool-start: func(name: string, arguments: string);
  notify-tool-output: func(text: string);
  notify-tool-done: func(name: string, summary: string);

  // 审批请求 (返回用户决策)
  request-approval: func(
    tool-name: string,
    arguments: string,
    allow-session-approval: bool,
  ) -> approval-decision;

  enum approval-decision { approved, approved-for-session, denied, abort }
}

interface session {
  use krew:types/messages.{chat-message};
  use krew:types/agents.{agent-snapshot};

  // 读完整历史 (cheap on host side, no copy until accessed)
  list-messages: func() -> list<chat-message>;
  // 增量订阅
  resource history-subscription {
    next: func() -> option<chat-message>;
  }
  subscribe: func() -> history-subscription;
  // 追加（host 写入并持久化）
  append-message: func(msg: chat-message);
  // Agent 元数据
  list-agents: func() -> list<agent-snapshot>;
  // 当前路由焦点（最后一个回答者等）
  last-respondent: func() -> option<string>;
}

interface bus {
  // 插件间事件总线（host 调度，避免 plugin 之间直连）
  // event-name 由 plugin manifest 声明（capability 必须）
  publish: func(event-name: string, payload: list<u8>);
  resource subscription {
    next: func() -> option<event>;
  }
  record event { name: string, payload: list<u8> }
  subscribe: func(pattern: string) -> subscription;
}

interface fs-bounded {
  // host 给 guest 的受限文件系统访问
  // 路径必须在 capability 声明的 root 之下
  read-file: func(path: string) -> result<list<u8>, fs-error>;
  write-file: func(path: string, bytes: list<u8>) -> result<_, fs-error>;
  list-dir: func(path: string) -> result<list<dir-entry>, fs-error>;
  record dir-entry { name: string, is-dir: bool }
  variant fs-error { not-found, permission-denied(string), io(string) }
}

interface config {
  // 拿 host 给我这个插件的配置子树（TOML 序列化为字节）
  get-plugin-config: func() -> option<list<u8>>;
}

interface log {
  enum level { trace, debug, info, warn, error }
  log: func(level: level, message: string);
  // host 端转交 tracing；插件名作为 target 自动加上
}

interface time {
  // host 已格式化好的本地时间字符串（按小时对齐，prompt cache 友好）。
  // 由 host 端 chrono::Local 生成，guest 不需要处理 timezone / locale。
  // 测试时 host 可注入 fake TimeView 实现严格 mock。
  now-local-hour-aligned: func() -> string;

  // ISO-8601 UTC 当前时间。给需要 UTC 的场景。
  now-utc: func() -> string;
}

// + wasi:http、wasi:cli、wasi:clocks、wasi:random 直接 add_to_linker
```

### 5.3 插件需实现的 worlds（按角色）

```wit
package krew:plugin@1.0.0;

// Provider 插件
// 设计原则：plugin 不持有任何 UI handle，仅做"协议组装 → HTTP → SSE 解析 → 事件流产出"。
// 调度（tool 调用、UI 渲染、usage 累计）一律由 host 拿到事件流后决定。
world provider {
  import krew:host/session;                   // 读 agent 元数据
  import krew:host/config;                    // 读 plugin-private 配置子树
  import krew:host/log;                       // 调试日志
  import wasi:http/outgoing-handler@0.3.0;    // 发请求
  import wasi:io/streams@0.3.0;               // 读 SSE incoming-body
  import wasi:clocks/wall-clock@0.3.0;        // 时间戳 / retry 计时

  export provider-iface;
}

interface provider-iface {
  use krew:types/messages.{
    chat-message, tool-call-info, server-tool-use-info, usage,
  };

  record sampling {
    temperature: option<f64>,
    top-p: option<f64>,
    top-k: option<u32>,
    max-tokens: option<u32>,
    frequency-penalty: option<f64>,
    presence-penalty: option<f64>,
    stop-sequences: list<string>,
  }

  record tool-spec {
    name: string,
    description: string,
    parameters-schema: string,                // JSON schema
  }

  // 与 crates/krew-llm/src/types.rs::StreamEvent + AgentEvent::Retrying 一一对应。
  // host 端拉流后：text/thinking-delta → TUI；tool-call → 派发 tool 系统；
  // server-tool-* → UI 标记；retrying → TUI 状态条；done → 累计 usage；error → 错误处理。
  record retry-info {
    attempt: u32,                              // 1-based
    max-attempts: u32,
    reason: string,                            // 例如 "rate limit (429)"
    delay-secs: f64,                           // 即将 sleep 的时长
  }

  variant provider-event {
    text-delta(string),
    thinking-delta(string),
    tool-call(tool-call-info),
    server-tool-start(string),                // tool 名（如 "web_search"）
    server-tool-done(server-tool-use-info),
    retrying(retry-info),                     // 重试前发出，TUI 据此显示 "重试中..."
    done(usage),
    error(string),
  }

  // Provider 主入口。返回事件流，host 异步消费。
  // 注意：返回 stream 而非 borrow<ui-stream>，让 plugin 完全不依赖 UI handle。
  chat-stream: func(
    self-agent-name: string,
    messages: list<chat-message>,
    tools: list<tool-spec>,
    sampling: sampling,
  ) -> stream<provider-event>;

  // 拉模型列表（config wizard 用）
  list-models: func() -> result<list<string>, string>;
}

// 路由插件
world router {
  import krew:host/session;
  export router-iface;
}

interface router-iface {
  enum addressee-kind { all, single, multiple, last-respondent }
  record addressee {
    kind: addressee-kind,
    targets: list<string>,
  }
  record parsed-input {
    addressee: addressee,
    body: string,
    is-whisper: bool,
  }

  parse-input: func(input: string, known-agents: list<string>) -> result<parsed-input, string>;
  parse-mentions: func(text: string, known-agents: list<string>, self-name: string) -> list<string>;
}

// Skill / Memory / Custom-command / Compact 各有独立 world，原理同上
```

> WIT 草案最终版会拆出 `xd-spec/changes/wasm-plugin-arch/specs/` 下的多个 spec 文件，跟仓库 xd-spec 工作流对齐。

---

## 6. 模块拆分与优先级

按"风险 × 价值"排序，**每一步都要可独立合回 main 主干（即使 host 还没切换默认实现）**。

### Wave 0 —— Prompt Contributor PoC（最小验证链路，必做前置）

**目标**：用最小、最低风险的功能切片，端到端跑通"host 加载 .wasm → 调插件方法 → 把结果嵌进生产代码路径"。**Wave 1 之前必做**——核心是验证 wasip3 工具链本身能跑通，先不引入 HTTP / 流式 / 消息历史 / tools 这些复杂面。

为什么先做这个，而不是直接 router PoC：
- router 同时引入多 `ChatMessage` 类型 + 多次调用序列，工具链报错时难归因。
- prompt-contributor 接口面是 `string -> string`（最多带 `agent_name` 参数），任何报错都能直接归到 toolchain 而非业务逻辑。
- 直接对应"自定义系统提示词注入"——pi 扩展生态最常用的 hook，krew-cli 必须从 day 1 提供。
- 接口在后续 Wave 2 的 `instructions` / `skill` / `memory` 都能复用，不是一次性投入。

**范围**：
| # | 工作 | 说明 |
|---|---|---|
| 0.1 | `rust-toolchain.toml` | 锁定 host 用 stable Rust + guest 用 nightly / 特定 stable 版本，明确 `wasm32-wasip3` target |
| 0.2 | `krew-host`（最小骨架） | wasmtime Engine 单例 + Store + .wasm 加载 + AOT 缓存到 `~/.krew/cache/aot/` |
| 0.3 | `krew-wit`（新 crate） | 定义首个 WIT 接口 `prompt-contributor.wit`，`wasmtime::component::bindgen!` 生成 host trait |
| 0.4 | `plugins/datetime/` | 把 `build_identity_prompt` 中的 `Current date: {now}` 抽成独立 .wasm；**guest 调 `krew:host/time::now-local-hour-aligned()` 拿格式化好的本地时间字符串**（保留原 chrono::Local 行为 + prompt cache 友好性）；同时 import `wasi:clocks/wall-clock@0.3.0` 用于验证 capability 通路（不参与产出逻辑） |
| 0.5 | **`krew-core` 接收预计算切片，不持 trait** | 在 `krew-core::agent` 定义纯数据类型 `Contribution { slot: Slot, priority: u32, body: String, ... }`（与 WIT 草案对齐，不依赖 wasmtime）。`build_identity_prompt` 改签名接收 `&[Contribution]`，按 slot + priority 嵌入片段。**core 完全保持同步**，不引入 async 也不引入 trait，规避 sync/async 边界冲突 |
| 0.6 | host 异步预收集 + 注入 | 在 `krew-cli` (host) 实现 `async fn collect_contributions(&self, agent_name: &str) -> Vec<Contribution>`：内部委托 `krew-host` plugin manager **异步**驱动 wasmtime 调 .wasm（wasmtime p3 native async + 插件 contribute() 可异步），把所有 plugin 的输出汇聚成 `Vec<Contribution>` 并按 slot + priority 排好序。**调用顺序**：krew-cli 在自己的 async 上下文里先 `let contribs = collect_contributions(name).await;`，再把切片作为参数传给同步的 `AgentRuntime::start_completion(..., &contribs)`（该函数本身保持 sync），由其内部传递给 `build_identity_prompt(..., &contribs)`。**绝对禁止在 core 的同步路径里 `block_on`** |
| 0.7 | 验收测试（**非严格等价**） | 三项检查：(a) 加载插件后 system prompt 出现 `Current date: <ts>` 行，且 `<ts>` 能被 `chrono::NaiveDateTime::parse_from_str(_, "%Y-%m-%d %H:%M (%A)")` 成功解析（**沿用既有格式 `%Y-%m-%d %H:00 (%A)`，例如 `2026-04-30 15:00 (Thursday)`**）；(b) 不加载该插件则该行消失，证明插件是 single source of truth；(c) 用 `TimeView` mock 注入固定时间，断言插件输出该时间字符串。**不要求和旧 hard-code 实现位级相等**——但格式必须与当前 `crates/krew-core/src/agent/mod.rs::build_identity_prompt` 调用方生成的 `now` chrono::Local 格式保持一致，避免悄悄变更 prompt 内容 |
| 0.8 | 性能基线 | 测会话首启 / 二启的 prompt assembly 延迟，确认 AOT 缓存确实生效（目标：首启 < 200ms，二启 < 5ms） |

**WIT 草案 v0**（最终版会落到 §5）：

```wit
package krew:plugin@1.0.0;

world prompt-contributor {
  import krew:host/log;                  // host 提供的日志接口（详见 §5.2）
  import krew:host/time;                 // host 提供的时间接口；mockable，本地时区已格式化
  import wasi:clocks/wall-clock@0.3.0;   // 仅为验证 capability 通路；不参与产出逻辑
  export contributor;
}

interface contributor {
  enum slot {
    core-identity,                       // 紧跟核心身份块（datetime、language 等）
    peer-collab,                         // 多 Agent 协作提示之后
    whisper-context,                     // 密语上下文之后
    custom,                              // host 不关心顺序的尾部
  }

  record contribution {
    slot: slot,
    priority: u32,                       // 同 slot 内拼接顺序，小的在前
    body: string,                        // 不含尾部换行，host 负责 join
    cache-friendly-until: option<string>, // ISO-8601；提示 host 此片段在该时刻前稳定（prompt cache 命中提示，host 可忽略）
  }

  contribute: func(agent-name: string) -> contribution;
}
```

**里程碑**：
- `cargo build --target wasm32-wasip3 -p datetime-plugin` 出 .wasm；
- host 启动后 `/agents` 看到的 system prompt 首部包含来自 datetime 插件的 `Current date: ...`；
- 卸下该插件 → 该行消失；
- 写一份 `docs/PLUGIN_AUTHORING.md` 雏形（仅"贡献者"这一类模式），后续 Wave 扩充。

**Wave 0 不做**：HTTP、流式、ChatMessage、Session、tool 调用、capability gating 完整落地（仅最小 capability 验证 `wasi:clocks` 能授给 guest）。

---

### Wave 1 —— Router PoC + HTTP/SSE 全链路验证（覆盖核心高风险路径）

Wave 0 只验证了 toolchain 和最简 string-in/string-out，**Wave 1 必须把 HTTP / SSE / tool-call event / cancellation / fuel/epoch 五件高风险事一次过**——这是 Codex review 明确指出 router-only PoC 不够的原因。

| # | 模块 | 说明 |
|---|---|---|
| 1.1 | `krew-host` 扩展 | 在 Wave 0 骨架上加：`wasmtime-wasi-http` 集成、`Store::set_fuel` / epoch interruption、AOT cache 热路径 |
| 1.2 | `plugins/router/` | 把 `router.rs` 整体编进 wasm32-wasip3，host 调用比对 |
| 1.3 | `plugins/mock-provider/` | **新增**。一个最小 Provider 插件，连本地 mock SSE 服务（`tests/util/mock_sse.rs` 起 hyper 服务），通过 `wasi:http@0.3.0/outgoing-handler` 发请求、读 SSE incoming-body stream、把 SSE 帧解析成 `provider-event` 流回 host |
| 1.4 | `tests/wasm_router.rs` | 100+ 解析 fixture，host 调 plugin vs 内置实现比对 |
| 1.5 | `tests/wasm_provider_mock.rs` | mock-provider 全事件验证：text-delta、thinking-delta、tool-call（含 thought-signature）、server-tool-start/done、done(usage)、error；ESC 取消立即生效；fuel 耗尽插件被 trap 但 host 存活 |
| 1.6 | `tests/wasm_capability.rs` | 故意配置 `http.allowed-hosts` 不含 mock 服务，验证 plugin 调 outgoing-handler 被 host 拒绝（trap → 友好错误） |

**里程碑**：
- `cargo test -p krew-host wasm_*` 全绿；
- 一个完整的"用户输入 → host parse-input → router plugin → mock provider plugin → mock SSE → 流式回到 host → TUI 显示"链路跑通；
- 验证 fuel/epoch 能在 < 100ms 内打断死循环 plugin；
- 验证 capability 校验拒绝越权 HTTP 请求。

**这个 Wave 走完，所有后续 Wave 的高风险已在最小代码量下证伪/证明。**

### Wave 2 —— 纯逻辑插件搬迁（低风险，迅速降 LOC）
| # | 模块 | 说明 |
|---|---|---|
| 2.1 | `plugins/memory/` | 完整搬 `krew-core::memory`，host 通过 `fs-bounded` 暴露 `.krew/memory/` |
| 2.2 | `plugins/skill/` | 搬 `krew-core::skill`（discovery + catalog）+ `activate_skill` tool |
| 2.3 | `plugins/custom-command/` | 搬 `krew-core::custom_command`（含 markdown frontmatter、Bash 预处理） |
| 2.4 | `plugins/instructions/` | AGENTS.md / CLAUDE.md 加载（搬 `krew-config::instructions`） |
| 2.5 | `plugins/compact/` | 搬 `krew-core::compact` + auto-compact 触发逻辑 |
| 2.6 | `plugins/dream/` | 搬 `krew-core::dream`（实验功能，正好检验插件 API） |

里程碑：host 默认走插件路径运行所有 v0.11 集成测试，行为等价。

### Wave 3 —— Provider 插件化（核心收益）
| # | 模块 | 说明 |
|---|---|---|
| 3.1 | `plugins/provider-openai-chat/` | 搬 `krew-llm::openai_chat`，HTTP 走 `wasi:http` |
| 3.2 | `plugins/provider-openai-responses/` | 搬 `krew-llm::openai_responses` |
| 3.3 | `plugins/provider-anthropic/` | 搬 `krew-llm::anthropic` |
| 3.4 | `plugins/provider-vertex-anthropic/` | 搬 `krew-llm::vertex_anthropic` |
| 3.5 | `plugins/provider-google/` | 搬 `krew-llm::google` |
| 3.6 | `krew-host` | 实现 `wasmtime-wasi-http` 集成 + TLS（hyper-rustls） |
| 3.7 | `plugins/list-models/` | `list_models` API 抽到独立插件，给 config wizard 用 |

里程碑：删除 `krew-llm` crate（逻辑全部迁出），host 不再直接发 LLM 请求。

### Wave 4 —— 多 Agent 编排 + 高级 tools
| # | 模块 | 说明 |
|---|---|---|
| 4.1 | `plugins/orchestrator/` | 搬 `agent_loop` 中除"工具执行"外的逻辑：reply_order 调度、A2A 路由、whisper 过滤、prepare_messages_for_agent |
| 4.2 | `plugins/tool-fetch-url/` | 搬 `fetch_url`，HTTP 走 `wasi:http` |
| 4.3 | `plugins/tool-web-search/` | 新增独立 web_search 工具（不依赖 Provider 原生 search） |
| 4.4 | `plugins/mcp-client/` | 搬 `krew-tools::mcp`，stdio 由 host 起子进程后把 fd 交 plugin（用 wasi:cli + wasi:io 流） |

里程碑：host 仅保留 TUI、Session IO、基础工具（read/write/edit/glob/grep/shell）、wasmtime 嵌入层，agent_loop 退化为"驱动 orchestrator 插件 + 渲染事件"。

### Wave 5 —— 收尾与发布
| # | 任务 |
|---|---|
| 5.1 | 插件清单 (`.krew/plugins.toml`) 设计：声明启用顺序、capability 授权 |
| 5.2 | 内嵌 vs 外置：常用插件 `include_bytes!`，自定义放 `.krew/plugins/*.wasm` |
| 5.3 | 插件版本协商：每个 plugin 上报支持的 `krew:host` 版本号，不匹配拒绝加载 |
| 5.4 | `/plugins` slash 命令：列出已加载插件 / capability / 状态 / token 占用 |
| 5.5 | 插件作者文档 `docs/PLUGIN_AUTHORING.md` |
| 5.6 | v1.0.0 发版 + Release Notes（无迁移指南：pre-GA 直接 breaking change） |

---

## 7. Host 必须提供的能力清单

按 WIT interface 归档，每项要在 Wave 1 收口前完成最小版：

| Interface | 内容 | 实现要点 |
|---|---|---|
| `wasi:http/outgoing-handler` | HTTP 请求 + SSE 分片读 | `wasmtime-wasi-http` + 共享 hyper Client，复用连接池 |
| `wasi:filesystem` | 受限文件系统 | preopen `cwd` + `~/.krew/`，禁止逃逸 |
| `wasi:cli/environment` | 读环境变量（API key） | 按插件 manifest 白名单过滤，避免泄露 SSH key 等 |
| `wasi:clocks` `wasi:random` | 时间 / 随机 | 直接用 `wasmtime_wasi::p3` 默认实现 |
| `krew:host/ui` | TUI 推送 + 审批请求 | host 端把事件丢进现有 `AgentEvent` 通道 |
| `krew:host/session` | 历史读写 + Agent 元数据 | 复用 `krew-storage` |
| `krew:host/bus` | 插件间事件 | host 端 `tokio::sync::broadcast`，按 capability 过滤 |
| `krew:host/fs-bounded` | 比 wasi:filesystem 更细的 sub-root 授权（仅给 memory / skill / instructions 用） | 每 plugin 注入自己的 root preopen |
| `krew:host/config` | 配置子树投递 | host 解析 settings.toml 后把 `[plugins.<id>]` 子表序列化为 TOML bytes |
| `krew:host/log` | 插件日志统一汇入 host tracing | 接收 (level, message)，host 端 tracing target 自动带上插件名 |
| `krew:host/time` | host 已格式化的本地时间 + UTC，可 mock | host 端用 chrono::Local；测试用 fake TimeView 注入 |

---

## 8. Capability / Manifest / 信任模型

### 8.1 两类插件 + 信任分层

| 类型 | 来源 | 加载策略 | 能力授权 |
|---|---|---|---|
| **内嵌（trusted-builtin）** | 仓库内 `plugins/*/`，`include_bytes!` 进 host | 默认全部启用，按 settings.toml `[plugins].load` 排序 | manifest 内能力默认全开（仍受 capability 字段约束，不能默写啥就有啥） |
| **外部（user-supplied）** | `.krew/plugins/*.wasm` / `~/.krew/plugins/*.wasm` | **默认拒绝加载**，必须先通过 `krew plugins trust <id>` 写入 `.krew/plugins.lock` | 每条能力首次请求时弹出确认（一次性允许 / 永久允许 / 拒绝） |

### 8.2 plugin.toml（与 .wasm 同目录或内嵌为 component custom section）

```toml
[plugin]
id = "krew.provider.openai-chat"
version = "1.0.0"
kind = "provider"                          # provider | router | skill | memory | command | compact | tool | mcp | prompt-contributor
host-version = ">=1.0.0, <2.0.0"

[capabilities]
http.allowed-hosts = ["api.openai.com", "*.litellm.example.com"]
env.allowed-vars = ["OPENAI_API_KEY"]    # 严禁通配；不允许 "*"
fs.read = []                              # 受 preopen root 约束
fs.write = []
bus.publish = ["provider.usage"]
bus.subscribe = []
clocks.wall-clock = true
# host.log 与 host.time 是 always-on 低风险能力，不需声明、不进 trust 流程：
#   - host.log 只能往 host tracing 写日志，不能读外部数据
#   - host.time 只暴露 host 已格式化的时间字符串，不暴露 timezone 之外的环境信息
# 任何插件都自动获得这两项；外部插件 trust 仪式也不会展示它们
```

**Always-on 低风险能力（不进 manifest / trust 流程）：**
- `krew:host/log` —— 写日志到 host tracing，无 side-effect、无信息泄露通路（host 端 tracing 接收方就在本进程），永远开放。
- `krew:host/time` —— 读 host 已格式化的本地时间 / UTC，read-only、无外部 IO，永远开放。

任何插件（含外部插件）都自动获得这两项能力；plugin.toml 不需要也不允许声明它们。如果未来发现 log/time 能被滥用形成侧信道（例如根据 log 限速反推用户活跃度），届时再升级为可声明能力。

### 8.3 .krew/plugins.lock（外部插件可信链）

`krew plugins trust <path>` 命令计算 SHA-256 + 解析 manifest，弹出能力清单让用户确认，然后写入：

```toml
# 自动生成，请勿手改。校验失败时 host 拒绝加载该插件。
[[plugin]]
id = "com.example.my-provider"
version = "0.3.1"
path = ".krew/plugins/my-provider.wasm"
sha256 = "8a3f...beef"
trusted-at = "2026-04-30T14:30:00Z"
trusted-capabilities = [
  "http://api.example.com",
  "env:MY_API_KEY",
]
```

**Host 加载流程：**
1. 读 manifest，校验 host-version 区间。
2. 内嵌插件：直接进入步骤 5。
3. 外部插件：算 .wasm 的 SHA-256，与 plugins.lock 对比；不匹配 → 拒绝并提示重新 trust。
4. 比对 manifest 的 `[capabilities]` 与 lock 的 `trusted-capabilities`；超出 trusted 范围的能力直接 strip（不授给 guest）。
5. 在 `Linker` 注册时按 capability 过滤：
   - `http`：用 `wasmtime-wasi-http::WasiHttpView::send_request` hook 校验 host 在 allow-list；wildcard 仅允许 `*.example.com` 形式的子域，绝对不允许 `*`。
   - `env`：包装 `wasi:cli/environment` 只暴露白名单变量名（精确匹配，不允许通配）。
   - `fs`：preopen 时只挂载授权目录；`fs-bounded` 用 sub-root 保护 `.krew/memory/`、`.krew/sessions/` 等。
   - `bus`：发布/订阅前检查 capability 字符串前缀。
   - `clocks` / `random`：开关型，要么全开要么全关。

不在白名单内的能力 = guest 调用 trap，host 翻译成 `PluginCapabilityViolation` 错误日志（不杀整个 host）。

### 8.4 已知不能防的事（必须公开承认）

- **API Key 出口劫持**：恶意 plugin 拿到 `OPENAI_API_KEY` 后，如果 `http.allowed-hosts` 包含真实 OpenAI host，理论上能正常调用——key 已经"出去"了，只是出去到合法目的地。**缓解**：列入 trust 流程的 prompt：高亮 "本插件可访问 OPENAI_API_KEY 并向 api.openai.com 发请求"，让用户判断这是否 reasonable。
- **侧信道**：plugin 通过响应时长、bus 事件等做侧信道泄露。v1.0 不防。
- **依赖供应链**：plugin 内嵌 crate 出现后门。v1.0 不防（v1.1 加签名后由 publisher 信任替代）。

`docs/PLUGIN_AUTHORING.md` 必须有"安全模型边界"专章，明确告诉用户 trust 一个外部插件意味着什么。

---

## 9. 配置变更

### 9.1 settings.toml 新增

```toml
[plugins]
# 插件加载顺序（同 kind 内部按声明顺序仲裁优先级）
load = [
  "krew.router.default",
  "krew.memory.default",
  "krew.skill.default",
  "krew.instructions.agents-md",
  "krew.compact.default",
  "krew.provider.openai-chat",
  "krew.provider.anthropic",
  "krew.provider.google",
  "krew.orchestrator.default",
]

# 外部插件路径（除内嵌外的额外查找路径）
extra_paths = ["~/.krew/plugins", ".krew/plugins"]

# 单个插件的私有配置（投递给该 plugin 的 get-plugin-config）
[plugins.config."krew.provider.openai-chat"]
# Provider 插件可任意定义自己的字段，host 不解释
default_base_url = "https://api.openai.com/v1"

# Agent 直接引用 Provider 插件（v1.0 起不再有顶层 [providers] 表）
[[agents]]
name = "gpt"
display_name = "GPT-5.5"
color = "#10a37f"
provider_plugin = "krew.provider.openai-chat"
model = "gpt-5.5"
api_key_env = "OPENAI_API_KEY"
```

> **TOML 命名约定**：所有 host 端解析的字段沿用 v0.x 既有的 **snake_case**（`display_name`、`api_key_env`、`reply_order` 等）。WIT 接口里的 kebab-case 是 WIT 语法要求，host bindgen 后映射回 Rust snake_case，对用户配置无影响。

### 9.2 不做向后兼容（pre-GA）

v0.x 尚未对外发布稳定承诺，重构期间项目方是唯一用户。**`.krew/settings.toml` 直接按新 schema 重写，不实现自动迁移、不保留旧 `[providers]` / `[[agents]].provider` 兼容路径，也不做"识别旧字段并提示"的软迁移。** 用法：用户删掉旧文件、跑 `krew config init` 重建，或手工按新 schema 重写。

**Strict 解析必须落地（与新 schema 同 Wave 落地）**：当前 `Config` / `AgentConfig` 等 struct 没有 `#[serde(deny_unknown_fields)]`，旧字段会被静默忽略——这与"按新 schema 重写"的承诺脱节。

**新 settings schema 实际上从 Wave 2 开始进入主路径**（Wave 0/1 的 datetime / router / mock-provider 都用 `include_bytes!` 内嵌，不依赖用户 `[plugins].load` 配置）。因此 **`#[serde(deny_unknown_fields)]` 必须在 Wave 2 第一个用户可见的 schema 变更时一起加上**——不能拖到 Wave 5，否则 Wave 2-4 期间用户改新 schema 时旧字段会被静默吞掉，问题难排查。

### 9.3 Host 与 Plugin 的配置职责边界

之前草案过激地"把所有配置解析下放 guest"。修正：**公共 schema 仍由 host 拥有 + 校验，仅 plugin 私有字段下放。**

| 配置块 | 解析方 | 校验方 | 理由 |
|---|---|---|---|
| `[settings]`（`reply_order`、`approval_mode`、`auto_compact_threshold` 等） | host | host | host 调度需要直接读 |
| `[[agents]]` 公共字段（`name`、`display_name`、`color`、`provider_plugin`、`model`、`enable_thinking`、`enable_web_search`、`tools`、`sampling`、`system_prompt`） | host | host | `reply_order` ↔ agent 名引用、agent ↔ `provider_plugin` 引用、`"all"` 保留字检查、name 唯一性都在 host 完成 |
| `[plugins].load` / `extra_paths` / `[plugins.config.<id>]` | host 拆分 | host 校验 manifest 兼容；子表内容不校验 | host 决定加载顺序；私有内容由 plugin 自解 |
| `[plugins.config.<id>]` 子表内部字段 | plugin | plugin | Provider 能加 `default_base_url` / `extra_headers` 等 provider-specific 字段，host 不必知道 |
| `[[allow_rules]]` / `[[deny_rules]]` / `[[ask_rules]]` | host | host | 权限规则跨 plugin 生效，必须 host 仲裁 |
| `[[mcp_servers]]` | host 加载 + 启动子进程 | host 校验 trust 字段 | 子进程生命周期跨 plugin |
| MCP 工具的私有 `annotations` | mcp-client plugin | plugin | plugin-specific |

**校验时机**：host 启动时跑 `Config::validate()`（保留当前 `crates/krew-config/src/lib.rs` 的实现），任何引用错误（不存在的 agent、不存在的 `provider_plugin`、`reply_order` 含未定义 agent）必须在加载任何 plugin 之前报错退出。

---

## 10. 性能 / 资源预算

| 维度 | 目标 | 策略 |
|---|---|---|
| 冷启动新增延迟 | < 80 ms | AOT cache (`Engine::precompile_component` → `.cwasm`)；常用插件首启编译，后续直接 mmap |
| 单插件常驻内存 | < 8 MB | 限制每个 Store 的 `MemoryType.maximum`；闲置插件用 `instance.drop` 释放 |
| 跨 ABI 调用开销 | < 50 µs / call | 高频路径（router、prepare）批量化，单次调用搬完整轮次而非 per-token |
| SSE 流式延迟 | 与 v0.x 一致（< 16ms 抖动） | 用 `wasi:http` 的 `incoming-body.stream` + WASI 0.3 `stream<u8>`，无中转缓冲 |
| 二进制总体积 | host < 18 MB；常用插件 .wasm < 600 KB / 个 | host 走 musl + lto；guest 用 `wasm-opt -Oz` |

---

## 11. 测试策略

### 11.1 等价测试
Wave 1～4 每搬一个模块，原 Rust 实现 + WASM 实现并行保留 2 个 release，跑同一组 fixture 比对输出。差异即 bug。

### 11.2 集成测试
- `tests/wasm_router.rs`：100+ 输入 fixture，host 调 plugin 解析 vs 内置实现比对。
- `tests/wasm_provider_anthropic.rs`：mock SSE server（`wiremock`）+ plugin 走 `wasi:http` 调它，校验 token 累加、tool_call 解析、retry。
- `tests/wasm_orchestrator.rs`：模拟 @all + A2A + whisper 完整流程。

### 11.3 模糊 / 安全
- guest trap 注入测试：故意 panic 的 .wasm，host 必须降级而非崩溃。
- capability 越权测试：未声明 `http.allowed-hosts` 的 plugin 调 `wasi:http` → 必须 trap。
- 资源耗尽：plugin 死循环 → wasmtime fuel / epoch interruption 介入终止。

---

## 12. 风险与未决问题

| # | 风险 | 缓解 |
|---|---|---|
| R1 | **押注 wasip3 / WASI 0.3 / wasmtime 37+**：Rust target Tier 3、wasmtime p3 标 experimental、生态库滞后 | (1) `rust-toolchain.toml` + `Cargo.lock` 锁版本快照；(2) Wave 0 用最小路径提前验证 toolchain 健康度；(3) wasmtime 升级专 PR + 全量等价测试再合入；(4) 真正的回退预案：v1.0 RC 阶段若 p3 阻塞，临时切 `wasi:http@0.2` binding（仅 host linker + krew-wit 入口改动，1-2 周）。**不预先实现双轨。** |
| R2 | 高频跨 ABI 调用开销（streaming path） | 用 WIT `stream<u8>` 一次借出，避免 per-token round-trip；profile 必做 |
| R3 | rustls-rustcrypto 不建议生产 → guest TLS 受限 | TLS 完全留 host，guest 永远拿明文流 |
| R4 | MCP stdio 子进程生命周期跨越 host/guest 边界 | host 起子进程，把 stdin/stdout 包成 `wasi:io/streams` 资源借给 plugin；plugin 退出时 host 负责 reap |
| R5 | 插件版本爆炸（5 Provider × N 版本 × 多语言模型） | host 内嵌 stable 版本；外置插件由社区维护；打 SemVer，host-version 区间约束 |
| ~~R6~~ | ~~配置自动迁移~~ | **已消除**：pre-GA 不做向后兼容，配置直接重写（见 §9.2） |
| R7 | 用户 API Key 经环境变量传给 plugin 时，能否被恶意 plugin 泄露给非授权 host | `env.allowed-vars` 白名单 + `http.allowed-hosts` 白名单双重约束；plugin 不能自由发起任意网络请求 |
| R8 | 插件 panic 导致历史污染 | 所有写入走 host append-message；plugin 报错时 host 回滚未持久化部分 |

**未决（Wave 1 立项时再敲定）：**
- 插件签名 / 完整性校验是否在 v1.0 引入？倾向"v1.0 不做，v1.1 加 ed25519 签名"。
- 是否提供"热重载"？倾向"v1.0 不做，重启即可"。
- 插件能否相互 RPC（A 调 B）？倾向"统一走 host bus，不允许直连"。

---

## 13. 工作流与协作

- **分支策略**：`arch/wasm-plugins` 为长期主干，每个 Wave 切 `refactor/wasm-<wave>-<topic>` 子分支，子分支合回 `arch/wasm-plugins`。重构完成后 `arch/wasm-plugins` 一次性合 main 并发 v1.0.0。
- **xd-spec 流程**：每个 Wave 至少立一个 xd-spec change（`xd-spec/changes/wasm-<wave>-<topic>/`），包含 specs + tasks + design。**`xd-spec/` 目录当前不存在，由 Wave 0 启动时一并初始化**（首个 change 即 `wasm-wave-0-prompt-contributor`，会顺带建立目录骨架与 lint 配置）。重构期 main 上的 v0.x 修复继续按 main 当时的流程，与本分支互不阻塞。
- **CI**：`arch/wasm-plugins` 上额外加 `wasm32-wasip3` 编译矩阵 + AOT cache 验证 + 上述等价测试；wasmtime 锁版本，升级要专门 PR。
- **进度追踪**：本文件末尾维护「进度看板」表格，每 Wave 收口时更新。

---

## 14. 进度看板

| Wave | 状态 | xd-spec change | 备注 |
|---|---|---|---|
| Wave 0 Prompt Contributor PoC | ⏳ Not started | — | toolchain & 最小插件链路验证；datetime 提示词作为首个插件 |
| Wave 1 Router + HTTP/SSE 全链路 | ⏳ | — | router + mock-provider；走通 HTTP/SSE/tool-call/cancel/fuel/capability |
| Wave 2 纯逻辑插件 | ⏳ | — | memory / skill / instructions / compact / dream / custom-command；同 Wave 加 `deny_unknown_fields` |
| Wave 3 Provider 插件化 | ⏳ | — | 5 Provider 全部 wasm 化，删 `krew-llm` |
| Wave 4 多 Agent 编排 + 高级 tools | ⏳ | — | orchestrator / fetch_url / web_search / mcp |
| Wave 5 收尾发布 | ⏳ | — | manifest、版本协商、文档、v1.0.0 |

---

## 15. 参考资料（已调研）

- WASI 0.3 native async in Wasmtime 37+: https://progosling.com/en/dev-digest/2026-02/wasi-0-3-wasmtime-37-native-async
- wasmtime::component bindgen: https://docs.wasmtime.dev/api/wasmtime/component/macro.bindgen.html
- wit-bindgen: https://github.com/bytecodealliance/wit-bindgen
- wasmtime-wasi-http: https://docs.wasmtime.dev/api/wasmtime_wasi_http/index.html
- WASI Roadmap: https://wasi.dev/roadmap
- Plugins with Rust and WASI Preview 2 (benw.is): https://benw.is/posts/plugins-with-rust-and-wasi
- Why Extism (对比，最终未选): https://dylibso.com/blog/why-extism/
- reqwest wasm32-wasip2 现状: https://github.com/seanmonstar/reqwest/issues/2979
- tokio WASI Preview 2 现状: https://github.com/tokio-rs/tokio/issues/6323
- Wasm 3.0 (Memory64 等) 标准: https://progosling.com/en/dev-digest/wasm-3-0-released
