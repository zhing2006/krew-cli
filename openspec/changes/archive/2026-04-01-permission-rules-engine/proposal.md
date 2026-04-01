## Why

当前审批系统只有三档全局模式（Suggest/AutoEdit/FullAuto）和简单的白名单匹配，缺乏细粒度控制。存在三个关键问题：

1. **FullAuto 模式没有安全底线**：开启后 LLM 可以静默修改 `.git/`、`.krew/` 等关键路径，没有任何保护机制
2. **没有 deny 规则**：无法禁止特定危险操作（如 `rm -rf`），只能通过白名单放行安全命令
3. **规则粒度不足**：shell 白名单仅支持前缀匹配，文件类工具没有路径级别的规则，`read_file` 也无法限制敏感文件的读取

参考 Claude Code 的权限系统设计，引入三维规则引擎（allow/deny/ask）+ bypass 免疫 + 通配符模式匹配。

## What Changes

- **BREAKING**：移除 `shell_allow_commands: Vec<String>` 配置字段，由新的 `[[allow_rules]]` 替代
- **BREAKING**：移除 `fetch_allow_domains: Vec<String>` 配置字段，由新的 `[[allow_rules]]` 替代
- 新增三维规则系统：`[[allow_rules]]`、`[[deny_rules]]`、`[[ask_rules]]`，每条规则包含 `tool`、`pattern`（可选）和 `reason`（可选）字段
- 新增 bypass 免疫机制：硬编码保护路径清单，即使 FullAuto 模式也必须用户确认
- 新增 `ToolApproval::Denied { reason }` 变体：deny 规则匹配后直接拒绝，不弹出审批 UI，LLM 收到拒绝原因后可以转达给用户
- 规则支持通配符模式匹配：shell 命令用 `*` 通配符，文件路径用 glob（`**`/`*`），域名用后缀匹配
- `read_file` 工具也纳入规则匹配范围，可以通过 deny/ask 规则限制敏感文件的读取
- 更新 config help 命令输出以反映新的配置结构
- 更新 README（中英文）、MANUAL、PDD、TDD 文档

## Capabilities

### New Capabilities

- `permission-rules`: 三维权限规则引擎核心能力，包括 allow/deny/ask 规则的解析、存储和匹配逻辑
- `bypass-immunity`: 硬编码保护路径的 bypass 免疫机制，确保关键文件在任何模式下都受保护

### Modified Capabilities

- `tool-approval-flow`: 审批管线重构为 8 步流程，集成 deny 规则直接拒绝、ask 规则强制确认、bypass 免疫检查
- `config-types`: 移除 `shell_allow_commands` 和 `fetch_allow_domains`，新增 `PermissionRule` 结构体和 `allow_rules`/`deny_rules`/`ask_rules` 字段
- `config-loading`: 加载和验证新的规则格式
- `config-validation`: 新增规则格式校验（工具名、pattern 语法）
- `approval-tui`: 审批 overlay 适配 deny 拒绝反馈的显示

## Impact

- **krew-config**：`Settings` 结构体破坏性变更，移除旧字段，新增规则相关类型
- **krew-core**：`check_tool_approval()` 完全重写，`ToolApproval` 枚举新增 `Denied` 变体，agent loop 处理 deny 结果
- **krew-tools**：`read_file` 工具需要将文件路径暴露给审批检查
- **krew-cli**：审批 overlay 需要处理 deny 决策的显示，config help 命令内容更新
- **文档**：README.md、README_EN.md、docs/MANUAL.md、docs/PDD.md、docs/TDD.md
- **配置兼容性**：用户需要将 `shell_allow_commands`/`fetch_allow_domains` 迁移到新的 `[[allow_rules]]` 格式
