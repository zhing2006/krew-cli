## ADDED Requirements

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
