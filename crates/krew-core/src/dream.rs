//! Memory consolidation ("dream") prompt construction.
//!
//! Builds a 3-phase consolidation prompt (Orient → Consolidate → Prune)
//! that instructs an agent to review and tidy memory files.

pub use crate::command::DreamScope;

/// Tools allowed during dream execution (whitelist).
///
/// Only file-operation tools are needed for memory consolidation.
/// Everything else (shell, fetch_url, MCP tools, etc.) is excluded.
pub const DREAM_ALLOWED_TOOLS: &[&str] = &["read_file", "write_file", "edit_file", "glob", "grep"];

/// Build the consolidation prompt for the given scope and agent.
///
/// The prompt instructs the agent to:
/// 1. Orient — discover what memory files exist
/// 2. Consolidate — merge duplicates, delete stale facts, fix contradictions
/// 3. Prune — keep MEMORY.md under 200 lines / 25KB
pub fn build_dream_prompt(scope: DreamScope, agent_name: &str) -> String {
    let (dirs, scope_label) = match scope {
        DreamScope::Global => (vec![global_dir_section()], "global"),
        DreamScope::Agent => (vec![agent_dir_section(agent_name)], "agent"),
        DreamScope::All => (
            vec![global_dir_section(), agent_dir_section(agent_name)],
            "all",
        ),
    };

    let dir_text = dirs.join("\n\n");
    let index_note = if scope == DreamScope::All {
        "\n\nNote: the two MEMORY.md files are independent indexes — do not merge them."
    } else {
        ""
    };

    format!(
        r#"You are performing memory consolidation (dream) for scope "{scope_label}".

Your task is to review, consolidate, and prune memory files. Work through 3 phases:

## Phase 1 — Orient

Use the `glob` tool to list directory contents, then `read_file` to read MEMORY.md and browse topic files.

Target directories:

{dir_text}{index_note}

## Phase 2 — Consolidate

- Merge duplicate or near-duplicate memory files (same topic, overlapping content).
- Delete files with stale or outdated facts that are no longer true.
- Fix contradictory entries — keep the most recent or most accurate version.
- Convert relative dates (e.g., "yesterday", "last week") to absolute dates.
- Remove entries that duplicate information already in the codebase (git history, README, etc.).
- To delete a file, use `write_file` to write an empty string (zero bytes). The system will automatically clean up empty files after consolidation. Do NOT write comments or markers into files you want to delete.

## Phase 3 — Prune index

Update MEMORY.md so that:
- Each entry is one line, under ~150 characters: `- [Title](file.md) — one-line hook`
- Total stays under 200 lines and 25KB
- Entries are organized semantically by topic, not chronologically
- Remove entries that point to deleted files

When you are done, provide a brief summary of what you changed."#
    )
}

fn global_dir_section() -> String {
    "- `.krew/memory/` — Global memory (shared by all agents)".to_string()
}

fn agent_dir_section(agent_name: &str) -> String {
    format!("- `.krew/memory/agents/{agent_name}/` — Your personal memory (private to you)")
}
