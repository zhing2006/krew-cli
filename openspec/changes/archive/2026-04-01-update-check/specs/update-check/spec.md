## ADDED Requirements

### Requirement: 启动时版本检查
`krew-cli` SHALL 在启动流程中（配置加载之后、显示 startup warnings 之前）检查是否有新版本可用。当 `settings.update_check` 为 `false` 时 SHALL 跳过检查。

#### Scenario: update_check 启用且有新版本
- **WHEN** `settings.update_check` 为 `true`，本地版本 `0.9.0`，npm 最新版本 `0.10.0`
- **THEN** SHALL 在 startup_warnings 中添加提示消息

#### Scenario: update_check 启用且已是最新
- **WHEN** `settings.update_check` 为 `true`，本地版本 `0.10.0`，npm 最新版本 `0.10.0`
- **THEN** SHALL 不添加任何版本相关的警告

#### Scenario: update_check 关闭
- **WHEN** `settings.update_check` 为 `false`
- **THEN** SHALL 跳过版本检查，不发起网络请求，不读取缓存

### Requirement: 版本更新提示消息格式
当检测到新版本时，提示消息 SHALL 格式为：
`New version v{latest} available (current: v{current}). Run: npm update -g @zhing2026/krew`

#### Scenario: 提示消息内容
- **WHEN** 本地版本为 `0.9.0`，最新版本为 `0.10.0`
- **THEN** 消息 SHALL 为 `New version v0.10.0 available (current: v0.9.0). Run: npm update -g @zhing2026/krew`

### Requirement: npm registry 查询
版本检查 SHALL 从 `https://registry.npmjs.org/@zhing2026/krew/latest` 发送 HTTP GET 请求，从 JSON 响应的 `version` 字段提取最新版本号。

#### Scenario: npm 请求成功
- **WHEN** npm registry 返回 `{"version": "0.10.0", ...}`
- **THEN** SHALL 提取 `version` 字段值 `0.10.0` 作为最新版本

#### Scenario: npm 请求失败
- **WHEN** npm registry 不可达或返回非 200 状态码
- **THEN** SHALL 将当前本地版本号作为 `latest_version` 写入缓存（触发 24h 冷却），静默跳过版本提示，不显示错误，不影响启动

### Requirement: 请求超时
npm registry 请求 SHALL 设置 2 秒超时。

#### Scenario: 请求超时
- **WHEN** npm registry 请求超过 2 秒未响应
- **THEN** SHALL 超时终止，将当前本地版本号作为 `latest_version` 写入缓存（触发 24h 冷却），静默跳过版本提示

### Requirement: 24 小时缓存机制
版本检查结果 SHALL 缓存到 `~/.krew/version_check.toml`。缓存文件包含 `latest_version`（String）和 `checked_at`（RFC3339 UTC 时间戳）。缓存有效期为 24 小时。

#### Scenario: 缓存未过期
- **WHEN** 缓存文件存在且 `checked_at` 距当前时间不超过 24 小时
- **THEN** SHALL 使用缓存的 `latest_version`，不发起网络请求

#### Scenario: 缓存已过期
- **WHEN** 缓存文件存在但 `checked_at` 距当前时间超过 24 小时
- **THEN** SHALL 发起 npm registry 请求获取最新版本，并更新缓存

#### Scenario: 缓存不存在
- **WHEN** 缓存文件不存在
- **THEN** SHALL 发起 npm registry 请求获取最新版本，并创建缓存文件

#### Scenario: 缓存文件损坏
- **WHEN** 缓存文件存在但无法反序列化
- **THEN** SHALL 视为缓存不存在，发起 npm registry 请求

#### Scenario: 缓存写入失败
- **WHEN** 缓存文件写入失败（权限等原因）
- **THEN** SHALL 静默忽略写入失败，不影响版本检查结果

#### Scenario: 请求失败后 24h 内不再重试
- **WHEN** 上次 npm 请求失败，缓存中 `latest_version` 等于当前本地版本，且 `checked_at` 距当前时间不超过 24 小时
- **THEN** SHALL 使用缓存值（与本地相同，不触发更新提示），不发起网络请求

### Requirement: 版本号比较
版本比较 SHALL 按 `.` 分割版本号为数字段，逐段解析为无符号整数后从左到右逐段比较。遇到第一个不相等的段时即决定大小关系：该段本地值小于远程值则判定需要更新，大于则判定不需要更新。所有段相等则判定不需要更新。段数不足时缺少的段视为 `0`。

#### Scenario: MINOR 版本落后
- **WHEN** 本地版本 `0.9.0`，最新版本 `0.10.0`
- **THEN** SHALL 判定需要更新

#### Scenario: PATCH 版本落后
- **WHEN** 本地版本 `1.2.3`，最新版本 `1.2.4`
- **THEN** SHALL 判定需要更新

#### Scenario: 版本相同
- **WHEN** 本地版本 `1.0.0`，最新版本 `1.0.0`
- **THEN** SHALL 判定不需要更新

#### Scenario: 本地版本更新
- **WHEN** 本地版本 `1.1.0`，最新版本 `1.0.0`
- **THEN** SHALL 判定不需要更新

#### Scenario: MAJOR 版本落后但后续段更大
- **WHEN** 本地版本 `1.10.0`，最新版本 `2.0.0`
- **THEN** SHALL 判定需要更新（第一段 `1 < 2` 即决定结果，不看后续段）

#### Scenario: 版本段数不同
- **WHEN** 本地版本 `1.0`，最新版本 `1.0.1`
- **THEN** 缺少的段 SHALL 视为 `0`，判定需要更新

#### Scenario: 版本号解析失败
- **WHEN** 版本号包含非数字字符（如 `1.0.0-beta`）
- **THEN** SHALL 静默跳过版本比较，不显示更新提示

### Requirement: Prompt 模式支持
当使用 `-p` 参数的 prompt 模式启动时，版本检查 SHALL 同样生效。检测到新版本时 SHALL 将警告输出到 stderr。

#### Scenario: prompt 模式有新版本
- **WHEN** 以 `krew -p "hello"` 启动，且有新版本可用
- **THEN** SHALL 在 stderr 显示版本更新警告
