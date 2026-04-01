## Context

当前审批系统由 `check_tool_approval()` 函数实现（`krew-core/src/agent/approval.rs`），使用 `ApprovalMode` 三档全局模式和两个白名单（`shell_allow_commands`、`fetch_allow_domains`）控制审批决策。系统缺乏：
1. 对关键路径（`.git/`、`.krew/`）的强制保护
2. 禁止特定操作的 deny 机制
3. 统一的、带通配符的规则匹配引擎

参考 Claude Code 源码（`src/utils/permissions/`），其使用 7 步审批管线、三维规则（allow/deny/ask）、bypass 免疫机制、以及基于通配符/glob 的模式匹配。本设计将这些概念适配到 krew-cli 的 Rust/TOML 架构中。

## Goals / Non-Goals

**Goals:**
- 引入 bypass 免疫机制，硬编码保护关键路径，FullAuto 模式下也必须确认
- 引入 deny 规则，匹配到直接拒绝（不弹 TUI），LLM 收到拒绝原因
- 引入 ask 规则，匹配到强制确认（FullAuto 也不跳过，bypass 免疫）
- 统一规则格式为 `[[allow_rules]]` / `[[deny_rules]]` / `[[ask_rules]]`，支持 `tool` + `pattern` + `reason`
- 规则支持通配符模式匹配：shell 命令用 `*` 通配符，文件路径用 glob，域名用后缀匹配
- `read_file` 也纳入规则匹配范围
- 完全移除旧的 `shell_allow_commands` 和 `fetch_allow_domains` 字段

**Non-Goals:**
- 不实现多来源规则优先级链（policy > user > project）——当前单配置源够用
- 不实现 AI 自动分类器（Claude Code 的 `auto` 模式）
- 不实现 hooks 系统（headless 模式暂不需要）
- 不修改 MCP 工具的 trust 机制——保持现有 `McpTrust` 枚举和 `check_mcp_approval()` 不变。但新的 deny/ask/allow 规则可以匹配 `mcp_` 前缀的工具名，且规则在审批管线前端优先评估，这意味着用户配置的 deny/ask 规则会覆盖 McpTrust 默认行为。这是预期设计：用户显式配置的安全策略优先级应高于 MCP trust 默认值

## Decisions

### Decision 1: 保护路径硬编码，不可配置

**选择**：保护路径清单硬编码在 Rust 代码中，不暴露为配置项。保护覆盖文件类工具和 shell 两个层面。

**原因**：如果保护路径可配置，LLM 可以修改配置文件来绕过保护。虽然我们信任自己的 LLM（本仙子），但不能保证所有 LLM provider 都值得信任。硬编码是唯一不可被 LLM 篡改的方式。

**替代方案**：在 settings.toml 中配置 → 被否决，因为 LLM 有能力修改配置文件。

**保护清单（参照 Claude Code `filesystem.ts`）**：

危险目录：
- `.git` — git 内部数据
- `.krew` — krew-cli 配置和会话
- `.vscode` — VS Code 配置
- `.idea` — JetBrains 配置
- `.claude` — Claude Code 配置

危险文件：
- `.gitconfig`, `.gitmodules` — git 配置
- `.bashrc`, `.bash_profile`, `.zshrc`, `.zprofile`, `.profile` — shell 配置
- `.env` — 环境变量/密钥

**两层保护**：
1. **文件类工具**（write_file/edit_file/read_file）：检查目标文件路径是否在保护清单中，匹配时返回 `NeedsApproval`
2. **Shell 工具**：硬编码一组不可覆盖的内置 deny 模式，拦截通过 shell 修改保护路径的常见操作。内置 shell deny 模式包括但不限于：`rm * .git*`、`rm * .krew*`、`rm * .env*` 等删除操作，以及涉及 `> .git*`、`> .krew*`、`> .env*` 的重定向写入。这些模式作为硬编码规则在用户 deny_rules 之前评估，不可覆盖。

**局限性声明**：Shell 的保护只能覆盖常见模式，无法穷举所有可能修改保护路径的 shell 命令（如变量展开、子 shell、管道组合等）。FullAuto 模式下 shell 访问具有固有风险。Claude Code 也未在 shell 层面做完整的路径保护。用户在使用 FullAuto 模式时应理解这一局限性。

### Decision 2: ToolApproval 新增 Denied 变体

**选择**：在 `ToolApproval` 枚举中新增 `Denied { reason: String }` 变体。

```rust
pub enum ToolApproval {
    Auto,
    NeedsApproval { allow_session_approval: bool },
    Denied { reason: String },
}
```

**原因**：deny 规则匹配后应直接拒绝，不走 TUI 审批流程。agent loop 收到 `Denied` 后，构造一个 `is_error: true` 的 `ToolResult`，将 reason 传给 LLM。LLM 可以用自然语言将拒绝原因转达给用户。

**agent loop 处理**：在 Phase 2（auto-approved 工具）之前新增一个 Phase，专门处理 denied 工具，跳过执行直接返回错误结果。

### Decision 3: 规则格式使用结构化 TOML

**选择**：使用 `[[allow_rules]]` / `[[deny_rules]]` / `[[ask_rules]]` TOML 数组表。

```toml
[[deny_rules]]
tool = "shell"
pattern = "rm -rf *"
reason = "禁止递归强制删除"

[[ask_rules]]
tool = "shell"
pattern = "npm publish *"
reason = "发布操作需要确认"

[[allow_rules]]
tool = "shell"
pattern = "cargo *"

[[allow_rules]]
tool = "read_file"
```

**结构体定义**：
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct PermissionRule {
    pub tool: String,
    pub pattern: Option<String>,
    pub reason: Option<String>,
}
```

**原因**：
- serde 直接反序列化，不需手写字符串解析器
- 错误提示清晰（TOML 层面报错）
- `reason` 字段天然支持 deny 拒绝原因
- 与现有 `[[agents]]`、`[[mcp_servers]]` 风格一致

**替代方案**：
- 字符串格式 `"shell:rm -rf *"` → 需要手写解析器，错误提示差
- 按工具分组的 map → 混合类型（true vs array），serde 复杂

### Decision 4: 模式匹配策略

**选择**：按工具类型选择匹配方式：

| 工具 | pattern 语义 | 匹配算法 |
|------|------------|---------|
| `shell` | 命令模式 | 通配符：`*` 转为正则 `.*`，支持转义 `\*` |
| `write_file`, `edit_file`, `read_file` | 文件路径 | glob 匹配：`**` 递归，`*` 单层，使用 `glob` crate |
| `fetch_url` | 域名/URL | 域名后缀匹配（与旧 `fetch_allow_domains` 行为一致） |
| 其他工具 | 无 | 无 pattern 时整个工具匹配，有 pattern 时忽略 |

**Shell 通配符实现**：将 `*` 替换为 `.*`，转义 regex 特殊字符，编译为 `Regex` 全匹配。特殊规则：尾部 ` *` 变为可选的 `( .*)?`，使 `cargo *` 同时匹配 `cargo` 和 `cargo build`。

**Shell 复合命令处理**：Shell 规则匹配 SHALL 复用现有 `shell_parse.rs` 中的 `extract_command_prefixes()` 逻辑。对于复合命令（`&&`、`||`、`;`、`|` 连接），先拆分为独立命令段，然后对每个命令段独立匹配规则。对于 deny 规则，任意一段匹配即整条命令被拒绝；对于 ask 规则，任意一段匹配即需要确认；对于 allow 规则，所有段都必须匹配才放行。

包含复杂构造（`$()`、反引号、重定向 `>`/`<`）的命令无法可靠解析时，三类规则的回退行为：
- **deny**：对原始整条命令字符串做通配符匹配（尽力拦截）
- **ask**：对原始整条命令字符串做通配符匹配（尽力确认）
- **allow**：不生效，回退到需要审批（不冒险放行）

**文件路径 glob 实现**：使用 `glob` crate 或手写简单 glob matcher。路径规则基于 cwd 相对路径匹配。Windows 路径先转为 POSIX 格式。

**文件路径归一化契约**：在做 glob 匹配之前，SHALL 对文件路径执行以下归一化：
1. Windows 反斜杠转为正斜杠
2. 绝对路径转为相对于 cwd 的相对路径
3. 规范化 `..` 和 `.` 路径段（resolve canonical path）
4. 去除前导 `./`
归一化后的路径用于 glob 匹配，确保 `.env`、`src/**`、`.git/**` 等规则行为一致。

### Decision 5: 审批管线重构为 8 步

**选择**：`check_tool_approval()` 重构为以下顺序：

```
Step 0: bypass 免疫检查（保护路径 + 内置 shell deny）
  → 文件类工具目标在 DANGEROUS_PATHS/FILES 中 → NeedsApproval（不可绕过）
  → shell 命令匹配内置危险模式 → Denied（不可覆盖）

Step 1: deny 规则检查（用户配置）
  → 匹配 deny_rules → Denied { reason }

Step 2: ask 规则检查
  → 匹配 ask_rules → NeedsApproval（bypass 免疫，FullAuto 也要确认）

Step 3: readonly 工具（无 deny/ask 规则匹配）→ Auto

Step 4: FullAuto 模式 → Auto

Step 5: allow 规则检查
  → 匹配 allow_rules → Auto

Step 6: session 缓存检查
  → 缓存命中 → Auto

Step 7: AutoEdit + 写文件工具 → Auto

Step 8: 默认 → NeedsApproval
```

**关键顺序保证**：
- Step 0, 1, 2 在 FullAuto 检查（Step 4）之前 → bypass 免疫
- deny 在 ask 之前 → deny 优先级最高
- allow 在 FullAuto 之后 → allow 只影响 Suggest/AutoEdit 模式下需要审批的工具

### Decision 6: read_file 纳入规则匹配

**选择**：`read_file` 工具的 `requires_approval()` 保持返回 `false`（默认不需审批），但 deny 和 ask 规则仍然对其生效。

**原因**：大多数文件读取是安全的，不应改变默认行为。但通过 deny/ask 规则，用户可以限制对 `.env`、密钥文件等敏感文件的读取。

**实现方式**：`check_tool_approval()` 在 Step 1（deny）和 Step 2（ask）中对所有工具（包括 readonly 工具）进行规则匹配。只有通过了 deny/ask 检查后，才在 Step 3 按 readonly 快速放行。

### Decision 7: 破坏性移除旧字段，启动时检测并警告

**选择**：完全移除 `shell_allow_commands` 和 `fetch_allow_domains`，不提供兼容层。但在配置加载时检测旧字段的存在，通过现有的 `startup_warnings` 机制显示迁移提示。

**原因**：
- 新的 `[[allow_rules]]` 完全覆盖旧功能
- 保留旧字段会造成两套规则系统共存的混乱
- 作为破坏性版本发布，在 CHANGELOG 中说明迁移方式

**deprecated field 检测实现**：在配置加载流程中，先读取原始 TOML 文本，检查是否包含 `shell_allow_commands` 或 `fetch_allow_domains` 键名。如果存在，向 `startup_warnings` 推送迁移提示消息，例如：`"配置字段 'shell_allow_commands' 已废弃，请迁移到 [[allow_rules]] 格式。参见 docs/MANUAL.md"`。这样用户升级后第一次启动就能看到黄色警告，而不是默默失效。

**迁移示例**：
```toml
# 旧格式
shell_allow_commands = ["cargo build", "git status"]
fetch_allow_domains = ["github.com"]

# 新格式
[[allow_rules]]
tool = "shell"
pattern = "cargo build *"

[[allow_rules]]
tool = "shell"
pattern = "git status *"

[[allow_rules]]
tool = "fetch_url"
pattern = "github.com"
```

### Decision 8: 默认 shell allow 规则的处理

**选择**：移除硬编码的 `DEFAULT_SHELL_ALLOW_COMMANDS` 常量。不再提供默认白名单。

**原因**：新的规则系统让用户可以精确控制。默认白名单（cat, ls, pwd 等）的概念被 FullAuto/AutoEdit 模式取代——如果用户信任 LLM 运行只读命令，应该选择相应的模式，而不是依赖硬编码白名单。

## Risks / Trade-offs

**[Breaking change] 用户配置需要迁移** → 在 README 和 CHANGELOG 中提供迁移指南，启动时通过 `startup_warnings` 检测旧字段并显示迁移提示。

**[性能] 每次工具调用都需匹配规则列表** → 规则数量通常很少（<20），线性匹配足够快。正则编译可考虑缓存。

**[glob crate 依赖] 引入新的 crate 依赖** → `glob` crate 轻量且广泛使用，可接受。也可考虑手写简单 glob 匹配避免依赖。

**[保护路径误伤] 硬编码路径可能阻止合理操作** → 保护路径触发的是 NeedsApproval 而非 Denied，用户仍然可以手动确认放行。这是安全与便利的合理平衡。
