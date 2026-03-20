## Context

krew-cli currently has a fixed set of slash commands defined in `SlashCommand` enum (`krew-core/src/command.rs`). The dispatch logic in `App::send_message()` checks `SlashCommand::from_input()` first; if it returns `None` and input starts with `/`, it falls through to normal message routing. The completion popup builds its slash command items from `SlashCommand::all_help()`.

Users cannot extend the command set. v0.3 adds custom slash commands following the Claude Code commands standard: Markdown files with YAML frontmatter stored in `.krew/commands/`, with bash preprocessing and argument substitution.

## Goals / Non-Goals

**Goals:**
- Users can define custom slash commands as `.md` files in `.krew/commands/`
- Commands support YAML frontmatter (`description`, `argument-hint`)
- Commands support subdirectory namespacing (`review/pr.md` → `/review:pr`)
- Commands support argument substitution (`$ARGUMENTS`, `$1`, `$2`)
- Commands support bash preprocessing (`!`command``) with output injection
- Custom commands appear in `/` completion popup (separate group below built-in)
- Custom commands appear in `/help` output
- Built-in commands always take priority over same-name custom commands
- Bash preprocessing errors produce inline error text, do not abort

**Non-Goals:**
- Global user-level commands (`~/.krew/commands/`) — deferred to later
- `allowed-tools` / `model` / `context: fork` frontmatter fields
- Interaction between custom commands and Agent Skills
- File-watching for hot-reload of commands (scan at startup only)

## Decisions

### D1: Custom commands module lives in `krew-core`

The discovery, parsing, frontmatter extraction, argument substitution, and bash preprocessing are all core logic, not TUI-specific. Place in `krew-core/src/custom_command/` with submodules:

- `mod.rs` — public types (`CustomCommand`, `CustomCommandRegistry`)
- `discovery.rs` — scan `.krew/commands/` directory, build registry
- `parser.rs` — parse frontmatter + body from markdown file
- `preprocessor.rs` — argument substitution + bash execution + output injection

**Alternative considered**: Put in `krew-cli`. Rejected because the command expansion logic (parsing, substitution, bash exec) is independent of TUI and should be testable without terminal dependencies.

### D2: Command dispatch flow

```
User input: /review:pr <args>
        │
        ▼
SlashCommand::from_input()  →  Some(builtin)?  →  execute built-in
        │ None
        ▼
CustomCommandRegistry::lookup("review:pr")  →  Some(cmd)?
        │ Some
        ▼
cmd.expand(args)            ← argument substitution
        │
        ▼
cmd.preprocess(expanded)    ← bash !`...` execution
        │
        ▼
parse_input(result)         ← normal @ routing
        │
        ▼
send as user message        ← enters agent loop
```

If both lookups fail, show "Unknown command" error (existing behavior).

**Alternative considered**: Merge custom commands into `SlashCommand::from_input()`. Rejected because it would couple the static enum with dynamic file-based commands and require `from_input` to take registry as parameter.

### D3: Frontmatter parsing — lightweight regex, no new dependency

Parse YAML frontmatter with simple string splitting (`---` delimiters) and manual key-value extraction for the two supported fields (`description`, `argument-hint`). No need for `serde_yaml` dependency for just two fields.

**Alternative considered**: Add `serde_yaml` crate. Rejected as overkill for two simple string fields.

### D4: Bash preprocessing uses `tokio::process::Command`

Execute `!`command`` blocks by spawning shell processes via `tokio::process::Command`. Capture stdout+stderr. On failure (non-zero exit or spawn error), replace the block with error text like `[Error: command failed (exit 1): stderr output]`. Continue with the rest of the command body.

Pattern matching: scan for `` !` `` followed by content and closing `` ` ``. Replace each match with its execution output.

### D5: Namespace mapping

Subdirectory structure maps to `:` separated command names:
- `commands/commit.md` → `/commit`
- `commands/review/pr.md` → `/review:pr`
- `commands/git/push.md` → `/git:push`

During discovery, walk the directory tree recursively, strip the base path and `.md` extension, replace path separators with `:`.

### D6: Completion popup integration

Extend `App::slash_command_items()` to append custom command items after built-in items. Custom commands use their `description` from frontmatter (or filename if no description). No visual separator needed — just append them; the `/` prefix filter handles the rest naturally.

The `/help` command similarly appends custom commands after the built-in list, with a "Custom commands:" subheading if any exist.

### D7: Startup scan timing

Scan `.krew/commands/` once during `App::new()` initialization, after config loading. Store the resulting `CustomCommandRegistry` in `App`. No file-watching or re-scanning during the session.

## Risks / Trade-offs

- **[Security: arbitrary bash execution]** → Mitigated by trust model: commands are user-authored project files. Same trust level as `.krew/settings.toml`. Users who clone repos should review `.krew/commands/` like any other config.
- **[Bash preprocessing blocks the event loop]** → Mitigated by running via `tokio::process::Command` (async). Long-running commands may still cause perceived delay, but won't freeze the TUI.
- **[No hot-reload]** → Users must restart krew-cli after editing command files. Acceptable for v0.3; file-watching can be added later if needed.
- **[Namespace collision with future built-in commands]** → Built-in always wins, so adding a new built-in `/foo` will shadow any custom `/foo`. This is the intended behavior.
