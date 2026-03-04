## ADDED Requirements

### Requirement: Approval overlay widget
TUI SHALL display a modal approval overlay when receiving `AgentEvent::ApprovalRequest`. The overlay SHALL show the tool name, arguments, and approval options.

#### Scenario: Shell approval display
- **WHEN** TUI receives ApprovalRequest for `shell("cargo test")`
- **THEN** overlay SHALL display: tool name, command, and selectable options

#### Scenario: Edit approval with diff
- **WHEN** TUI receives ApprovalRequest for `edit_file` with a unified diff
- **THEN** overlay SHALL display the diff with colored rendering (green for additions, red for deletions) above the approval options

### Requirement: Approval keyboard shortcuts
The approval overlay SHALL support keyboard shortcuts for quick decisions:
- `y` — Approve (execute this time)
- `a` — Approve for session (don't ask again)
- `n` or `Esc` — Deny (skip, tell LLM)
- `Ctrl+C` — Abort (stop agent turn)
- `Enter` — Confirm currently selected option
- `↑`/`↓` — Navigate options

#### Scenario: Quick approve
- **WHEN** user presses `y` while approval overlay is shown
- **THEN** ReviewDecision::Approved SHALL be sent via the oneshot channel

#### Scenario: Quick deny
- **WHEN** user presses `Esc` while approval overlay is shown
- **THEN** ReviewDecision::Denied SHALL be sent via the oneshot channel

#### Scenario: Approve for session
- **WHEN** user presses `a` while approval overlay is shown
- **THEN** ReviewDecision::ApprovedForSession SHALL be sent

#### Scenario: Abort
- **WHEN** user presses `Ctrl+C` while approval overlay is shown
- **THEN** ReviewDecision::Abort SHALL be sent

### Requirement: Approval overlay layout
The approval overlay SHALL render in the viewport area (replacing the input area temporarily). Layout:

```
  ⚡ shell("cargo test") — approve?

  › Yes, proceed                         (y)
    Yes, don't ask again this session    (a)
    No, skip this tool                   (n)
```

#### Scenario: Visual layout
- **WHEN** approval overlay is displayed
- **THEN** it SHALL show tool call info at top, followed by selectable options with keyboard hints

### Requirement: Approval queue
When multiple tool calls in a single round all require approval, the overlay SHALL process them sequentially (one at a time). Each decision is sent immediately; the next approval appears after the current one is resolved.

#### Scenario: Two approvals in sequence
- **WHEN** LLM returns `[shell("mkdir foo"), shell("touch foo/bar")]` and both need approval
- **THEN** overlay SHALL show first command, wait for decision, then show second command

### Requirement: Approval overlay dismissal
After the user selects an option, the overlay SHALL dismiss and return control to the normal input area.

#### Scenario: Dismiss after approval
- **WHEN** user presses `y` to approve
- **THEN** overlay SHALL disappear and input area SHALL be restored

### Requirement: Diff preview in approval
For write_file and edit_file approvals, the overlay SHALL display the file changes using the diff rendering system (colored unified diff).

#### Scenario: write_file shows content preview
- **WHEN** approval for write_file is shown
- **THEN** the file content (or first N lines) SHALL be displayed above the options

#### Scenario: edit_file shows diff
- **WHEN** approval for edit_file is shown with a unified diff
- **THEN** the diff SHALL be rendered with green (+) and red (-) coloring
