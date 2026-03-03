## 1. Storage Layer (krew-storage)

- [x] 1.1 Add `StorageError::Toml` variant for TOML parse errors; add `chrono`, `serde`, `uuid` dependencies to krew-storage Cargo.toml if missing
- [x] 1.2 Define `SessionFile` and `MessageEntry` serde structs in `session_file.rs` for TOML serialization/deserialization (matching TDD §3.6.1 format)
- [x] 1.3 Implement `save_session()`: accept session data, serialize to TOML, atomic write (`.tmp` + rename) to `.krew/sessions/<id>.toml`, auto-create directory
- [x] 1.4 Implement `load_session()`: read TOML file, deserialize into `SessionFile`, return structured data
- [x] 1.5 Implement `list_sessions()`: scan `.krew/sessions/*.toml`, parse each file's `[session]` table for summary (id, agents, updated_at, first message preview), sort by `updated_at` desc, skip corrupted files
- [x] 1.6 Write unit tests for session save/load/list round-trip, corrupted file handling, and empty directory

## 2. Input History Persistence (krew-storage)

- [x] 2.1 Create `history_file.rs` in krew-storage with `escape_line()` / `unescape_line()` helper functions (newline ↔ `\\n`, backslash ↔ `\\\\`)
- [x] 2.2 Implement `load_history()`: read `.krew/history`, unescape each line, return `Vec<String>`
- [x] 2.3 Implement `save_history()`: write all entries (escaped) to `.krew/history` (full rewrite for truncation)
- [x] 2.4 Implement `append_history_entry()`: open file with `OpenOptions::append`, write single escaped line
- [x] 2.5 Write unit tests for escape/unescape round-trip, load/save/append, file-not-found handling

## 3. Session Struct Enhancement (krew-core)

- [x] 3.1 Add `Serialize`/`Deserialize` derives to `Session` struct; add `Session::new()` constructor that generates UUID and sets timestamps
- [x] 3.2 Export `session_file` and `history_file` modules properly from `krew-storage::lib.rs`

## 4. App Session Integration (krew-cli)

- [x] 4.1 Add `session_id: String` and `session_dir: PathBuf` fields to `App`; generate UUID in `App::new()`; create `.krew/sessions/` directory
- [x] 4.2 Implement `App::save_session()` helper: build `SessionFile` from App state (map `krew_llm::ChatMessage` → `MessageEntry`), call `krew_storage::session_file::save_session()`
- [x] 4.3 Call `save_session()` after user message send (in `handle_submit`) and after agent response complete (`AgentEvent::Done`)
- [x] 4.4 Display session ID (first 8 chars) in startup header

## 5. Input History Integration (krew-cli)

- [x] 5.1 On `App::new()`: call `load_history()`, take last `input_history_limit` entries, populate `App.history`, call `save_history()` to truncate file
- [x] 5.2 Modify `history_push()`: after in-memory push, call `append_history_entry()` to persist

## 6. /new Command Enhancement (krew-cli)

- [x] 6.1 Modify `execute_clear` → `execute_new`: save current session (if non-empty), clear messages + token usage, generate new session ID, clear screen, show new header with new session ID

## 7. /resume Command Implementation (krew-cli)

- [x] 7.1 Implement `execute_resume`: call `list_sessions()`, display numbered list (date/time, agents, first message preview), show "No saved sessions" if empty
- [x] 7.2 Implement session selection: after listing, the next numeric input selects a session — load it, restore messages + token usage, show confirmation message

## 8. --resume CLI Argument (krew-cli)

- [x] 8.1 Process `cli.resume` in `async_main`: if `--resume` without ID → load most recent session; if `--resume <id>` → find session by prefix match; on failure → warn and start new session
- [x] 8.2 When resuming via `--resume`, skip new session creation and instead populate App with loaded session data

## 9. Final Verification

- [x] 9.1 Run `cargo fmt --all` and `cargo clippy --all-targets --all-features -- -D warnings`
- [x] 9.2 Run `cargo test` and verify all new and existing tests pass
- [x] 9.3 Manual smoke test: start → chat → /quit → /resume → verify messages restored; test --resume flag; test /new; test input history across sessions
