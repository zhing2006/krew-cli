## 1. Input Routing (krew-core)

- [x] 1.1 Add `validate_addressee(addressee: &Addressee, agent_names: &[String]) -> Result<()>` to `krew-core::router` — pass for All/LastRespondent, check Single(name) exists in list
- [x] 1.2 Add unit tests for `validate_addressee` — valid name, invalid name, @all, LastRespondent

## 2. Slash Command Execution (krew-cli)

- [x] 2.1 Refactor `App::send_message()` — replace hardcoded `/quit` with `SlashCommand::from_input()` dispatch, add `/exit` to `SlashCommand` enum or handle as alias
- [x] 2.2 Implement `/help` — insert command list above viewport via `insert_before`
- [x] 2.3 Implement `/agents` — display agent table with `[name]`, display_name, provider/model, token count (placeholder 0)
- [x] 2.4 Implement `/clear` — clear terminal visible content and re-display header
- [x] 2.5 Implement placeholder commands — `/new`, `/resume`, `/compact` show "Not yet implemented" message
- [x] 2.6 Handle unknown commands — `/unknown` shows error message above viewport
- [x] 2.7 Add `render::insert_system_message()` helper for system/info/error messages above viewport

## 3. Input Parsing Integration (krew-cli)

- [x] 3.1 Integrate `parse_input()` into `send_message()` for non-slash input — parse addressee, validate against config agents, handle errors
- [x] 3.2 Upgrade echo display — show route tag prefix `[→ @all]` / `[→ @gpt]` / `[→ last]` in echo reply

## 4. Completion Popup

- [x] 4.1 Create `completion.rs` module — define `ActivePopup` enum, `CompletionState` struct, filtered items logic (prefix match)
- [x] 4.2 Implement `sync_popup()` — detect `/` at start of first line (SlashCommand popup) or `@` token at cursor (AgentName popup), auto-open/close
- [x] 4.3 Implement popup keyboard handling — ↑/↓ navigate with wrap, Tab inserts selected item, Enter executes (slash) or inserts (agent), Esc closes
- [x] 4.4 Implement popup rendering — render completion list in viewport below the bottom separator, replacing status bar area, selected item in cyan bold
- [x] 4.5 Update `render_input_viewport()` layout — when popup active, replace status bar with popup rows, calculate dynamic viewport height including popup
- [x] 4.6 Implement Tab/Enter insertion — for slash commands replace entire input line, for agent names replace the `@token` with `@selected_name `

## 5. Integration & Polish

- [x] 5.1 Run `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings`, fix all warnings
- [x] 5.2 Manual testing — verify all scenarios from specs: @all/@name/invalid agent, /help /agents /clear /quit, slash and agent completion popup
