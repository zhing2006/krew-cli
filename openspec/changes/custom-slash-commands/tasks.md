## 1. Core Types & Module Structure

- [x] 1.1 Create `krew-core/src/custom_command/mod.rs` with `CustomCommand` struct (name, description, argument_hint, body) and `CustomCommandRegistry` (HashMap lookup, list method)
- [x] 1.2 Create `krew-core/src/custom_command/parser.rs` — parse a single `.md` file: split frontmatter (`---` delimiters), extract `description` and `argument-hint` fields, return `CustomCommand`
- [x] 1.3 Add unit tests for parser: full frontmatter, partial frontmatter, no frontmatter, empty file

## 2. Discovery

- [x] 2.1 Create `krew-core/src/custom_command/discovery.rs` — recursively scan `.krew/commands/` directory, map file paths to `:` separated command names, parse each file, build `CustomCommandRegistry`
- [x] 2.2 Add unit tests for discovery: flat files, nested directories, non-md files ignored, missing directory returns empty registry

## 3. Argument Substitution

- [x] 3.1 Implement `CustomCommand::substitute_args(&self, args: &str) -> String` — replace `$ARGUMENTS` with full arg string, `$1`/`$2`/... with positional args, unmatched positional → empty string
- [x] 3.2 Add unit tests for argument substitution: $ARGUMENTS, positional, missing positional, no arguments

## 4. Bash Preprocessing

- [x] 4.1 Create `krew-core/src/custom_command/preprocessor.rs` — async function to scan text for `` !`command` `` patterns, execute each via `tokio::process::Command`, replace with stdout output
- [x] 4.2 Implement error handling: non-zero exit → `[Error: command failed (exit N): stderr]`, spawn failure → `[Error: failed to execute: cmd]`
- [x] 4.3 Add unit tests for preprocessing: successful command, failed command, no bash blocks, multiple blocks

## 5. Command Dispatch Integration

- [x] 5.1 Wire `CustomCommandRegistry` into `App` — scan during `App::new()` init, store as field
- [x] 5.2 Modify `App::send_message()` — after `SlashCommand::from_input()` returns `None`, check custom command registry; if found, run expand → preprocess → `parse_input()` → send as message
- [x] 5.3 Update `App::execute_help()` — append custom commands section with "Custom commands:" subheading after built-in commands

## 6. Completion Popup Integration

- [x] 6.1 Extend `App::slash_command_items()` — append custom command items (name with `/` prefix, description from frontmatter) after built-in items
- [x] 6.2 Verify completion filtering works for custom commands including `:` namespace separator

## 7. End-to-End Testing

- [x] 7.1 Create test `.krew/commands/` fixtures with sample command files (flat + nested)
- [x] 7.2 Add integration test: custom command discovery → expansion → argument substitution → routing through `parse_input()`
