## Why

用户无法知道是否有新版本可用，可能一直使用过时版本而错过重要更新和修复。启动时自动检查 npm registry 并提示更新，能以最小侵入性帮助用户保持最新。

## What Changes

- 新增启动时版本检查：从 npm registry 查询 `@zhing2026/krew` 最新版本，与本地编译版本比较
- 24 小时缓存机制：检查结果缓存到 `~/.krew/version_check.toml`，避免每次启动都发网络请求
- 同步检查 + 2 秒超时：缓存过期时同步请求 npm，超时或失败静默跳过
- 自写 semver 比较：按 `.` 分段逐段比数字，解析失败静默跳过
- 新增 `update_check` 配置项（默认 `true`）：用户可关闭检查
- 更新双语 MANUAL 和 `config help` 命令输出

## Capabilities

### New Capabilities
- `update-check`: 启动时从 npm registry 检查最新版本，与本地版本比较，落后时显示更新提示，含 24h 缓存和开关配置

### Modified Capabilities
- `config-types`: Settings 结构体新增 `update_check: bool` 字段（默认 true）
- `config-help-command`: config help 输出新增 `update_check` 字段说明

## Impact

- **krew-config**: `Settings`、`RawSettings`、`UserSettings` 新增字段，`config.example.toml` 更新
- **krew-cli**: 新增 `update_check` 模块（npm 请求、缓存读写、版本比较）；`main.rs` 启动流程调用；`config help` 文本更新
- **docs**: `MANUAL.md`、`MANUAL_CN.md` 新增配置项说明
- **依赖**: `krew-cli` 新增 `reqwest` workspace 依赖（workspace 已有），无新外部 crate
