## Context

krew-cli 当前在启动 banner 中显示编译时版本号，但无法告知用户是否有更新可用。用户通过 npm 安装（`npm install -g @zhing2026/krew`），最新版本信息可从 npm registry 获取。

已有基础设施：
- `reqwest`（with rustls）用于 HTTP 请求
- `chrono` 用于时间处理
- `toml` 用于配置序列化/反序列化
- `startup_warnings` 队列 + `show_warning()` 用于启动时显示警告
- `Settings` / `RawSettings` / `UserSettings` 分层配置系统

## Goals / Non-Goals

**Goals:**
- 启动时检查 npm registry 上的最新版本
- 本地版本落后时在 startup warnings 中提示用户
- 24 小时缓存避免频繁网络请求
- 用户可通过 `update_check = false` 关闭
- 网络失败、超时、版本号解析失败均静默跳过，不影响正常使用

**Non-Goals:**
- 不做自动更新/自动下载
- 不支持 GitHub Release 作为版本源
- 不支持非 npm 安装方式的升级提示
- 不引入新的外部 crate（semver 等）

## Decisions

### Decision 1: 版本源选择 npm registry

从 `https://registry.npmjs.org/@zhing2026/krew/latest` 获取最新版本。

**理由**: 升级命令本身就是 npm（`npm update -g @zhing2026/krew`），版本源和升级渠道一致最合理。npm registry 无认证限制，响应快。

**备选**: GitHub Release API — 有未认证 rate limit（60/h），且仓库可能私有。

### Decision 2: 同步检查 + 24h 缓存 + 失败冷却

缓存存在且未过期时读本地文件（零延迟）。缓存过期时同步请求 npm，设 2 秒超时。

请求失败或超时时，将当前本地版本号写入缓存作为 `latest_version`。这样 24h 内再次启动时缓存未过期，且 `latest == current` 不触发更新提示，也不发起网络请求。无需引入额外的失败状态字段。

**理由**: 有了 24h 缓存后，99% 的启动是纯本地读取。每天最多一次 2 秒超时风险，可以接受。比异步方案简单得多，不需要处理"检查结果回来时用户已开始交互"的问题。失败冷却复用现有缓存结构，零额外复杂度。

**备选**: 异步非阻塞检查 — 需要跟踪用户交互状态来决定是否插入警告，复杂度高但收益小。

### Decision 3: 自写版本比较

按 `.` 分割版本号，逐段解析为 `u32` 比较。任何段解析失败（如 pre-release 标签 `1.0.0-beta`）直接静默跳过，不做比较。

**理由**: krew-cli 使用纯数字 semver（`MAJOR.MINOR.PATCH`），不需要完整 semver 规范（pre-release、build metadata）。避免引入新依赖。

### Decision 4: 缓存文件格式和位置

缓存在 `~/.krew/version_check.toml`，格式：
```toml
latest_version = "0.10.0"
checked_at = "2026-04-01T10:30:00Z"
```

**理由**: 复用 `~/.krew/` 目录（已由 user config 使用），TOML 格式与项目一致，`chrono` 的 `DateTime<Utc>` 直接序列化/反序列化 RFC3339。

### Decision 5: 代码组织与集成点

在 `krew-cli` 新增 `update_check.rs` 模块，包含：
- `async fn check_for_update(enabled: bool) -> Option<String>` — 异步主入口，内部执行缓存读取、npm 请求、版本比较，返回警告消息或 None
- `fn compare_versions(current: &str, latest: &str) -> Option<Ordering>` — 纯同步版本比较，解析失败返回 None
- 缓存读写的内部函数

`krew-cli` crate 需新增 `reqwest` workspace 依赖。

**集成点**: `main.rs` 中 tokio runtime 建立之后、prompt/TUI 分支之前。调用方式为 `runtime.block_on(check_for_update(...))`。返回的警告 push 到一个共享的 `warnings: Vec<String>` 列表（与 config_warnings 合并），之后在分支处分发：
- **Prompt 模式（`-p`）**: 遍历 warnings 输出到 stderr（与现有 config_warnings 打印逻辑一致）
- **TUI 模式**: warnings 灌入 `app.startup_warnings`，由 TUI 启动时统一显示

### Decision 6: npm registry 响应解析

仅需从 JSON 响应中提取 `version` 字段。使用 `serde_json::Value` 做简单提取，无需定义完整结构体。

## Risks / Trade-offs

- **[风险] npm registry 不可达** → 2 秒超时 + 写入当前版本作为缓存（24h 冷却）。仅当缓存写入也失败时才可能在下次启动重复超时。
- **[风险] npm 上版本号格式异常** → 解析失败静默跳过，不 panic、不警告。
- **[风险] 缓存文件被手动修改/损坏** → 反序列化失败时视为缓存不存在，重新请求。
- **[取舍] 同步阻塞 vs 异步** → 选择简单的同步方案，接受每 24h 最多 2s 延迟。
- **[取舍] 不支持 pre-release 版本比较** → 当前项目不使用 pre-release 标签，可以接受。
