## ADDED Requirements

### Requirement: ReviewDecision enum
`krew-core` SHALL define a `ReviewDecision` enum with variants: `Approved`, `ApprovedForSession`, `Denied`, `Abort`.

#### Scenario: ReviewDecision variants
- **WHEN** constructing each ReviewDecision variant
- **THEN** all four variants SHALL be available: Approved (execute this time), ApprovedForSession (don't ask again this session), Denied (skip, tell LLM), Abort (stop agent turn)

### Requirement: ApprovalMode config
`krew-config` SHALL define an `ApprovalMode` enum with variants: `Suggest` (default), `AutoEdit`, `FullAuto`. This SHALL be configurable via `approval_mode` field in settings.toml.

#### Scenario: Default mode
- **WHEN** settings.toml does not specify `approval_mode`
- **THEN** SHALL default to `Suggest`

#### Scenario: Config parsing
- **WHEN** settings.toml contains `approval_mode = "auto-edit"`
- **THEN** SHALL parse as `ApprovalMode::AutoEdit`

### Requirement: Approval policy evaluation
Agent loop SHALL evaluate whether a tool call requires approval based on `ApprovalMode` and `tool.requires_approval()`:

| Tool approval flag | Suggest | AutoEdit | FullAuto |
|---|---|---|---|
| `requires_approval() = false` | auto | auto | auto |
| `requires_approval() = true` (write tools) | approve | auto | auto |
| shell tool | approve | approve | auto |

#### Scenario: Suggest mode write tool
- **WHEN** ApprovalMode is Suggest and tool is write_file
- **THEN** agent loop SHALL send ApprovalRequest and await decision

#### Scenario: AutoEdit mode write tool
- **WHEN** ApprovalMode is AutoEdit and tool is edit_file
- **THEN** agent loop SHALL execute without approval

#### Scenario: AutoEdit mode shell
- **WHEN** ApprovalMode is AutoEdit and tool is shell
- **THEN** agent loop SHALL send ApprovalRequest and await decision

#### Scenario: FullAuto mode shell
- **WHEN** ApprovalMode is FullAuto and tool is shell
- **THEN** agent loop SHALL execute without approval

### Requirement: AgentEvent ApprovalRequest variant
`AgentEvent` SHALL include an `ApprovalRequest` variant carrying: `tool_name: String`, `arguments: String`, `diff: Option<String>` (unified diff for edit operations), and `respond: oneshot::Sender<ReviewDecision>`.

#### Scenario: Approval event sent
- **WHEN** agent loop determines a tool needs approval
- **THEN** it SHALL send `AgentEvent::ApprovalRequest` with a oneshot sender and await the receiver

#### Scenario: Agent loop blocks
- **WHEN** ApprovalRequest is sent
- **THEN** the agent loop SHALL block (await the oneshot receiver) until TUI sends a ReviewDecision

### Requirement: Approval session cache
Agent loop SHALL maintain a session-scoped approval cache. When user selects `ApprovedForSession`, future calls to the same tool with the same arguments SHALL skip approval.

#### Scenario: Cached approval
- **WHEN** user approves `shell("cargo test")` with ApprovedForSession
- **AND** agent later calls `shell("cargo test")` again
- **THEN** agent loop SHALL skip approval and execute directly

#### Scenario: Different args not cached
- **WHEN** user approves `shell("cargo test")` with ApprovedForSession
- **AND** agent later calls `shell("rm -rf /tmp/foo")`
- **THEN** agent loop SHALL still require approval

### Requirement: Denied tool result
When user selects `Denied`, agent loop SHALL return a ToolResult with `is_error: true` and content explaining the user denied the operation. The LLM can then decide an alternative approach.

#### Scenario: Denied result
- **WHEN** user denies a shell command
- **THEN** ToolResult SHALL be `{ content: "User denied execution of shell(\"rm -rf /tmp\"). Try a different approach.", is_error: true }`

### Requirement: Abort stops agent turn
When user selects `Abort`, agent loop SHALL stop the current tool-call cycle and emit `AgentEvent::Error` with a message that the user aborted.

#### Scenario: Abort behavior
- **WHEN** user selects Abort on a shell approval
- **THEN** agent loop SHALL stop processing remaining tool calls and emit Error event
## Requirements
### Requirement: MCP tool approval policy
Agent loop SHALL evaluate MCP tool approval based on `McpTrust` and tool annotations:

| MCP trust | Annotation | Suggest | AutoEdit | FullAuto |
|---|---|---|---|---|
| `auto` | any | auto | auto | auto |
| `confirm` | read_only=true | auto | auto | auto |
| `confirm` | destructive=true | approve | approve | auto |
| `confirm` | no annotations | approve | approve | auto |

#### Scenario: MCP tool trust=auto skips approval
- **WHEN** ApprovalMode is Suggest and MCP tool has trust=auto
- **THEN** agent loop SHALL execute without approval regardless of annotations

#### Scenario: MCP tool trust=confirm with read_only annotation
- **WHEN** ApprovalMode is Suggest and MCP tool has trust=confirm and read_only_hint=true
- **THEN** agent loop SHALL execute without approval

#### Scenario: MCP tool trust=confirm with destructive annotation
- **WHEN** ApprovalMode is Suggest and MCP tool has trust=confirm and destructive_hint=true
- **THEN** agent loop SHALL send ApprovalRequest and await decision

#### Scenario: MCP tool trust=confirm without annotations
- **WHEN** ApprovalMode is Suggest and MCP tool has trust=confirm and no annotations
- **THEN** agent loop SHALL send ApprovalRequest and await decision (safe default)

### Requirement: MCP tool session approval cache
When user selects `ApprovedForSession` for an MCP tool, future calls to the same MCP tool SHALL skip approval for the remainder of the session.

#### Scenario: MCP tool session approval
- **WHEN** user approves MCP tool `mcp__github__create_issue` with ApprovedForSession
- **AND** agent later calls the same MCP tool
- **THEN** agent loop SHALL skip approval and execute directly

