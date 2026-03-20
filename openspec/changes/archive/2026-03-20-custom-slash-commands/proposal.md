## Why

当前 krew-cli 的 slash 命令全部是内置硬编码的，用户无法根据自己的工作流定义快捷命令。参照 Claude Code 的 custom commands 标准，v0.3 需要让用户能在 `.krew/commands/` 目录中以 Markdown 文件定义自定义 slash 命令，实现「命令 = 可带 bash 预处理的 prompt 模板」。

## What Changes

- Add custom command discovery: scan `.krew/commands/` at startup, build command registry
- Add command file parser: YAML frontmatter (`description`, `argument-hint`) + Markdown body
- Add namespace support: subdirectories map to `:` separated names (`review/pr.md` → `/review:pr`)
- Add argument substitution: `$ARGUMENTS`, `$1`, `$2` etc. in command body
- Add bash preprocessing: `!`command`` blocks executed before sending, output replaces the block
- Integrate custom commands into `/` completion popup with a separate "Custom" group
- Integrate custom commands into `/help` output
- Route expanded command text through existing `parse_input()` for `@agent` addressing
- Built-in commands take priority over custom commands with the same name
- Bash preprocessing errors produce error text inline (do not abort the command)

## Capabilities

### New Capabilities
- `custom-commands`: Custom slash command loading, parsing, preprocessing, argument substitution, and execution
- `bash-preprocessing`: Shell command execution within command templates (`!`cmd``) with output injection

### Modified Capabilities
- `slash-commands`: Add custom command integration into command dispatch (unknown `/` input checks custom registry before showing error)
- `completion-popup`: Add custom commands as a separate group in `/` completion

## Impact

- **krew-core**: New `custom_commands` module for discovery, parsing, preprocessing, execution
- **krew-cli**: TUI completion popup changes to include custom commands group
- **krew-core/slash_commands**: Dispatch logic extended to check custom command registry before returning "unknown command" error
- **Dependencies**: May need a YAML frontmatter parser (or reuse existing `serde_yaml` / lightweight parsing)
- **File system**: Reads `.krew/commands/` directory at startup
