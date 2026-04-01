## MODIFIED Requirements

### Requirement: Approval policy evaluation
Agent loop SHALL 通过 `check_tool_approval()` 按以下 8 步管线评估工具调用是否需要审批：

**Step 0 — Bypass 免疫检查**：
- 对 `write_file`、`edit_file`、`read_file` 工具，检查目标路径是否在硬编码保护清单中。匹配时返回 `NeedsApproval`，任何模式都不可绕过。
- 对 `shell` 工具，检查命令是否匹配内置危险模式（硬编码，不可覆盖）。匹配时返回 `Denied`。

**Step 1 — Deny 规则检查（用户配置）**：遍历 `deny_rules`，匹配时返回 `Denied { reason }`。

**Step 2 — Ask 规则检查**：遍历 `ask_rules`，匹配时返回 `NeedsApproval`（bypass 免疫，FullAuto 也不跳过）。

**Step 3 — Readonly 工具**：`requires_approval() = false` 的工具返回 `Auto`。

**Step 4 — FullAuto 模式**：返回 `Auto`。

**Step 5 — Allow 规则检查**：遍历 `allow_rules`，匹配时返回 `Auto`。

**Step 6 — Session 缓存检查**：缓存命中时返回 `Auto`。

**Step 7 — AutoEdit + 写工具**：ApprovalMode 为 AutoEdit 且工具为 write_file/edit_file 时返回 `Auto`。

**Step 8 — 默认**：返回 `NeedsApproval`。

#### Scenario: Suggest mode write tool（保持不变）
- **WHEN** ApprovalMode is Suggest and tool is write_file, no rules match
- **THEN** agent loop SHALL send ApprovalRequest and await decision

#### Scenario: FullAuto mode shell（保持不变）
- **WHEN** ApprovalMode is FullAuto and tool is shell, no deny/ask rules match
- **THEN** agent loop SHALL execute without approval

#### Scenario: Deny rule blocks shell
- **WHEN** deny_rules contains `tool = "shell", pattern = "rm *"` and tool call is `shell("rm foo.txt")`
- **THEN** agent loop SHALL return Denied with the rule's reason, without prompting user

#### Scenario: Ask rule forces approval in FullAuto
- **WHEN** ApprovalMode is FullAuto and ask_rules contains `tool = "shell", pattern = "npm publish *"`
- **AND** tool call is `shell("npm publish")`
- **THEN** agent loop SHALL send ApprovalRequest (ask rules are bypass-immune)

#### Scenario: Allow rule auto-approves in Suggest mode
- **WHEN** ApprovalMode is Suggest and allow_rules contains `tool = "shell", pattern = "cargo *"`
- **AND** tool call is `shell("cargo build --release")`
- **THEN** agent loop SHALL execute without approval

#### Scenario: Bypass immunity before deny rules
- **WHEN** deny_rules contains `tool = "write_file"` and tool call targets `.git/config`
- **THEN** SHALL return NeedsApproval (bypass immunity at Step 0, before deny at Step 1)

#### Scenario: Built-in shell deny in FullAuto
- **WHEN** ApprovalMode is FullAuto and shell command is `rm -rf .git`
- **THEN** SHALL return Denied (built-in shell deny pattern, unconfigurable)

### Requirement: Denied tool result
When `check_tool_approval()` returns `Denied { reason }`, agent loop SHALL skip tool execution and return a ToolResult with `is_error: true` and content including the deny reason. The LLM SHALL receive the reason so it can inform the user.

#### Scenario: Denied result with reason
- **WHEN** deny rule matches `shell("rm -rf /tmp")` with reason "禁止递归强制删除"
- **THEN** ToolResult SHALL be `{ content: "Tool denied: 禁止递归强制删除", is_error: true }`

#### Scenario: Denied result without reason
- **WHEN** deny rule matches `shell("dd if=/dev/zero")` with no reason configured
- **THEN** ToolResult SHALL be `{ content: "Tool denied by deny rule.", is_error: true }`

### Requirement: Approval session cache
Agent loop SHALL maintain a session-scoped approval cache. When user selects `ApprovedForSession`, the cache key depends on the tool type:
- **shell**: cache by extracted command prefix (e.g. `cargo build`); same prefix with different flags auto-approved, different subcommands still need approval
- **fetch_url**: cache by URL host; same host auto-approved, different hosts still need approval
- **other tools** (write_file, edit_file, read_file, MCP): cache by tool name; all future calls to the same tool auto-approved
- **IMPORTANT**: Session cache SHALL NOT override bypass immunity checks (Step 0). Protected path tool calls SHALL always require confirmation regardless of cache state.

#### Scenario: Shell cached by prefix（保持不变）
- **WHEN** user approves `shell("cargo build --release")` with ApprovedForSession
- **AND** agent later calls `shell("cargo build -p krew-core")`
- **THEN** agent loop SHALL skip approval (same prefix `cargo build`)

#### Scenario: Cache does not bypass protected paths
- **WHEN** user approves `edit_file` with ApprovedForSession
- **AND** agent later calls `edit_file` targeting `.krew/settings.toml`
- **THEN** agent loop SHALL still require approval (bypass immunity)

## ADDED Requirements

### Requirement: Agent loop denied phase
Agent loop SHALL add a new processing phase for denied tool calls. Denied tools SHALL be separated from approval-needed and auto-approved tools, and SHALL immediately produce error ToolResults without execution or TUI interaction.

#### Scenario: Denied tools processed without UI
- **WHEN** LLM returns `[shell("rm -rf /"), shell("ls")]` and first is denied by rule
- **THEN** agent loop SHALL produce error result for first tool without UI, and process second tool normally

#### Scenario: Multiple denied tools
- **WHEN** LLM returns three tools, two of which match deny rules
- **THEN** both denied tools SHALL produce error results, and the non-denied tool SHALL proceed through normal approval flow
