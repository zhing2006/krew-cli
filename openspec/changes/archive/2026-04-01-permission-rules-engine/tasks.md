## 1. Config 类型变更 (krew-config)

- [x] 1.1 定义 `PermissionRule` 结构体（tool, pattern, reason 字段），派生 Deserialize/Serialize/Clone/Debug
- [x] 1.2 修改 `Settings` 结构体：移除 `shell_allow_commands` 和 `fetch_allow_domains` 字段，新增 `allow_rules`、`deny_rules`、`ask_rules` 三个 `Vec<PermissionRule>` 字段（默认空 Vec）
- [x] 1.3 移除 `DEFAULT_SHELL_ALLOW_COMMANDS` 常量和 `default_shell_allow_commands()` 函数
- [x] 1.4 更新 `Config::default()` 实现，使用新字段
- [x] 1.5 更新 `RawSettings` 和 `UserSettings` 中对应字段的定义和合并逻辑（规则列表拼接合并）
- [x] 1.6 实现 deprecated field 检测：在配置加载时检查原始 TOML 是否包含 `shell_allow_commands` 或 `fetch_allow_domains`，存在时返回警告消息列表，由 `krew-cli` 推送到 `startup_warnings` 显示黄色警告

## 2. Config 校验 (krew-config)

- [x] 2.1 在 `Config::validate()` 中新增权限规则工具名校验（已知工具名：shell, write_file, edit_file, read_file, fetch_url, glob, grep, activate_skill, run_agent，以及 mcp_ 前缀）
- [x] 2.2 新增 shell 通配符 pattern 语法校验（检测误用正则语法）
- [x] 2.3 更新现有校验测试，适配移除旧字段后的结构变化

## 3. Bypass 免疫机制 (krew-core)

- [x] 3.1 定义 `DANGEROUS_DIRECTORIES` 和 `DANGEROUS_FILES` 硬编码常量
- [x] 3.2 实现 `is_dangerous_path(file_path: &str) -> bool` 函数，支持目录段匹配和文件名匹配，大小写不敏感，Windows 路径归一化
- [x] 3.3 定义 `BUILTIN_SHELL_DENY_PATTERNS` 硬编码常量，覆盖通过 shell 删除/写入保护路径的常见命令模式
- [x] 3.4 实现 `is_dangerous_shell_command(command: &str) -> Option<String>` 函数，复用 shell_parse 拆分逻辑，对每个命令段检查内置危险模式，返回拒绝原因
- [x] 3.5 编写单元测试：覆盖 is_dangerous_path（.git/、.krew/、.bashrc、.env、正常路径、Windows 路径、大小写变体）和 is_dangerous_shell_command（rm .git、rm .krew、rm .env、正常 rm target/、复合命令中含危险段、重定向写入如 `echo x > .env`、`cat foo > .git/config`）

## 4. 规则匹配引擎 (krew-core)

- [x] 4.1 实现 shell 通配符匹配函数：`*` 转正则 `.*`，支持 `\*` 转义，尾部 ` *` 可选，全字符串锚定匹配
- [x] 4.2 实现文件路径归一化函数：Windows 反斜杠转正斜杠、绝对路径转 cwd 相对路径、规范化 `..`/`.` 路径段、去除前导 `./`
- [x] 4.3 实现文件路径 glob 匹配函数：在归一化路径上支持 `**` 递归、`*` 单层、精确文件名
- [x] 4.4 实现域名后缀匹配函数（从旧 fetch_allow_domains 逻辑迁移）
- [x] 4.5 实现 shell 复合命令的规则匹配逻辑：复用 shell_parse.rs 拆分命令段，deny 任一段匹配即拒绝，ask 任一段匹配即确认，allow 所有段都匹配才放行；复杂构造下 deny/ask 做整串匹配，allow 不生效
- [x] 4.6 实现 `matches_rule(tool_name, arguments, rule) -> bool` 统一入口，按工具类型分派匹配策略
- [x] 4.7 编写规则匹配的单元测试：覆盖 shell 通配符各场景、复合命令匹配（deny 任一段、ask 任一段、allow 需全段、复杂构造下 deny/ask 整串匹配、复杂构造下 allow 不生效）、glob 各场景（含路径归一化）、域名匹配、无 pattern 匹配整工具、工具名不匹配

## 5. 审批管线重构 (krew-core)

- [x] 5.1 在 `ToolApproval` 枚举中新增 `Denied { reason: String }` 变体
- [x] 5.2 重写 `check_tool_approval()` 函数为 8 步管线：Step 0 bypass 免疫（文件路径 + 内置 shell deny）→ Step 1 用户 deny → Step 2 ask → Step 3 readonly → Step 4 FullAuto → Step 5 allow → Step 6 cache → Step 7 AutoEdit → Step 8 默认
- [x] 5.3 更新 `check_tool_approval()` 的函数签名：接收 `deny_rules`、`ask_rules`、`allow_rules` 参数（替代旧的 `shell_allow_commands`、`fetch_allow_domains`）
- [x] 5.4 修改 agent loop 新增 denied phase：分离 denied 工具调用，直接产生 error ToolResult 不走 TUI
- [x] 5.5 更新 `cache_session_approval()` 逻辑，确保保护路径不被缓存绕过
- [x] 5.6 编写审批管线的单元测试：覆盖 8 步管线各场景、deny 优先于 ask、ask bypass 免疫、bypass 免疫不受缓存影响、内置 shell deny 不可覆盖

## 6. TUI 适配 (krew-cli)

- [x] 6.1 在 agent_display 中新增 denied 反馈行显示（红色 `✗ Denied: reason` 或 `✗ Denied by rule`）
- [x] 6.2 修改审批 overlay：ask 规则触发时显示 reason 文本
- [x] 6.3 更新 `AgentEvent::ApprovalRequest` 增加可选 reason 字段（来自 ask 规则）

## 7. Config Help 命令更新 (krew-cli)

- [x] 7.1 更新 `/config help` 命令输出内容：移除 `shell_allow_commands` 和 `fetch_allow_domains` 的说明，新增 `allow_rules`、`deny_rules`、`ask_rules` 的格式说明和示例

## 8. 文档更新

- [x] 8.1 更新 README.md（中文）：权限配置部分替换为新规则格式说明
- [x] 8.2 更新 README_EN.md（英文）：同步中文 README 的权限配置变更
- [x] 8.3 更新 docs/MANUAL.md：更新审批系统详细说明、配置示例、迁移指南
- [x] 8.4 更新 docs/PDD.md：更新产品设计中权限相关的章节
- [x] 8.5 更新 docs/TDD.md：更新技术设计中审批管线、规则匹配引擎的架构说明
