//! Persistent memory system for cross-session agent learning.
//!
//! Two-layer storage:
//! - Global: `.krew/memory/` — shared by all agents (user/project/reference types)
//! - Per-Agent: `.krew/memory/agents/{name}/` — agent-specific (feedback type)

use std::fs;
use std::path::Path;

/// Maximum number of lines to load from MEMORY.md.
const MAX_MEMORY_LINES: usize = 200;

/// Maximum number of bytes to load from MEMORY.md.
const MAX_MEMORY_BYTES: usize = 25_000;

/// Check if a normalized file path is inside `.krew/memory/`.
///
/// Used by the approval carve-out to auto-approve memory file operations.
/// Expects a forward-slash normalized path (relative to cwd).
pub fn is_memory_path(normalized_path: &str) -> bool {
    let lower = normalized_path.to_lowercase();
    lower == ".krew/memory" || lower.starts_with(".krew/memory/")
}

/// Build the complete memory prompt for injection into system prompt.
///
/// - When `has_tools` is true: includes full read/write instructions + index content
/// - When `has_tools` is false: includes only index content (read-only)
/// - Returns `None` if directory creation fails or no content to inject
pub fn load_memory_prompt(agent_name: &str, cwd: &str, has_tools: bool) -> Option<String> {
    // Guard against empty or missing cwd — don't create directories at
    // relative paths when no working directory is set.
    if cwd.is_empty() {
        return None;
    }

    let base = Path::new(cwd).join(".krew").join("memory");
    let agent_dir = base.join("agents").join(agent_name);

    // Ensure directories exist.
    if fs::create_dir_all(&base).is_err() || fs::create_dir_all(&agent_dir).is_err() {
        return None;
    }

    let mut parts: Vec<String> = Vec::new();

    // Inject full instructions template only when agent has tools.
    if has_tools {
        let template = MEMORY_PROMPT_TEMPLATE.replace("{{agent_name}}", agent_name);
        parts.push(template);
    }

    // Load Global MEMORY.md.
    let global_memory_path = base.join("MEMORY.md");
    if let Some(content) = read_and_truncate(&global_memory_path)
        && !content.is_empty()
    {
        parts.push(format!("## Global Memory\n\n{content}"));
    }

    // Load Per-Agent MEMORY.md.
    let agent_memory_path = agent_dir.join("MEMORY.md");
    if let Some(content) = read_and_truncate(&agent_memory_path)
        && !content.is_empty()
    {
        parts.push(format!("## Your Memory\n\n{content}"));
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
}

/// Read a file and truncate by line count and byte size.
///
/// Returns `None` if the file does not exist. Returns `Some("")` if empty.
/// Appends a warning line when truncation occurs.
fn read_and_truncate(path: &Path) -> Option<String> {
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return None,
    };

    if content.is_empty() {
        return Some(String::new());
    }

    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();
    let mut truncated_by_lines = false;
    let mut truncated_by_bytes = false;

    // Step 1: Truncate by line count.
    let selected = if total_lines > MAX_MEMORY_LINES {
        truncated_by_lines = true;
        &lines[..MAX_MEMORY_LINES]
    } else {
        &lines[..]
    };

    // Step 2: Truncate by byte size — find last complete line within limit.
    let mut result = String::new();
    let mut byte_count: usize = 0;
    let mut included_lines = 0;

    for (i, line) in selected.iter().enumerate() {
        let line_bytes = line.len() + if i > 0 { 1 } else { 0 }; // +1 for newline
        if byte_count + line_bytes > MAX_MEMORY_BYTES {
            truncated_by_bytes = true;
            break;
        }
        if i > 0 {
            result.push('\n');
        }
        result.push_str(line);
        byte_count += line_bytes;
        included_lines = i + 1;
    }

    // Append warning if truncated.
    if truncated_by_lines || truncated_by_bytes {
        let path_str = path.file_name().unwrap_or_default().to_string_lossy();
        let mut reasons = Vec::new();
        if truncated_by_lines {
            reasons.push(format!("{total_lines} lines (limit: {MAX_MEMORY_LINES})"));
        }
        if truncated_by_bytes {
            let total_bytes = content.len();
            reasons.push(format!(
                "{total_bytes} bytes (limit: {MAX_MEMORY_BYTES}), {included_lines} lines loaded"
            ));
        }
        result.push_str(&format!(
            "\n\n⚠ {path_str} is {}. Only part of it was loaded.",
            reasons.join("; ")
        ));
    }

    Some(result)
}

/// Memory instructions template injected into system prompt for agents with tools.
const MEMORY_PROMPT_TEMPLATE: &str = r#"# Memory

You have a persistent, file-based memory system at `.krew/memory/`. This directory already exists — write to it directly with `write_file` (do not run mkdir or check for its existence).

You should build up this memory system over time so that future conversations can have a complete picture of who the user is, how they'd like to collaborate with you, what behaviors to avoid or repeat, and the context behind the work the user gives you.

If the user explicitly asks you to remember something, save it immediately as whichever type fits best. If they ask you to forget something, find and remove the relevant entry.

## Storage Structure

There are two layers of memory storage:

- **Global** (`.krew/memory/`): Shared by all agents — for facts about the user, project, and external references
- **Personal** (`.krew/memory/agents/{{agent_name}}/`): Private to you — for behavioral feedback specific to you

## Types of memory

| Type | Scope | Where to save | Description |
|------|-------|---------------|-------------|
| `user` | Global | `.krew/memory/` | Information about the user's role, goals, preferences, and knowledge |
| `project` | Global | `.krew/memory/` | Ongoing work, goals, initiatives, bugs, or incidents not derivable from code/git |
| `reference` | Global | `.krew/memory/` | Pointers to external resources (issue trackers, dashboards, docs) |
| `feedback` | Personal | `.krew/memory/agents/{{agent_name}}/` | Guidance the user has given YOU about how to approach work |

### When to save each type

- **user**: When you learn details about the user's role, preferences, responsibilities, or knowledge
- **project**: When you learn who is doing what, why, or by when. Convert relative dates to absolute dates
- **reference**: When you learn about resources in external systems and their purpose
- **feedback**: When the user corrects your approach OR confirms a non-obvious approach worked

## What NOT to save

- Code patterns, conventions, architecture, file paths, or project structure — derivable from code
- Git history or recent changes — use `git log` / `git blame`
- Debugging solutions — the fix is in the code
- Anything already documented in project config files
- Ephemeral task details or current conversation context

## How to save memories

Saving a memory is a two-step process:

**Step 1** — Write the memory to its own file (e.g., `user_role.md`, `feedback_testing.md`) as plain Markdown in the appropriate directory.

**Step 2** — Add a pointer to that file in the corresponding `MEMORY.md` index. Each entry should be one line, under ~150 characters: `- [Title](file.md) — one-line hook`.

- Keep `MEMORY.md` concise — lines after 200 will be truncated
- Organize memory semantically by topic, not chronologically
- Update or remove memories that turn out to be wrong or outdated
- Do not write duplicate memories — check if there is an existing memory you can update

## When to access memories

- When memories seem relevant, or the user references prior-conversation work
- You MUST access memory when the user explicitly asks you to check, recall, or remember
- Use `read_file` to read specific topic files when you need their full content

## Important notes

- Memory records can become stale. Verify against current state before acting on them
- If a recalled memory conflicts with current information, trust what you observe now — update or remove the stale memory"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── read_and_truncate tests ──

    #[test]
    fn read_and_truncate_normal_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("MEMORY.md");
        fs::write(
            &path,
            "- [User](user.md) — data scientist\n- [Project](proj.md) — deadline",
        )
        .unwrap();
        let result = read_and_truncate(&path).unwrap();
        assert!(result.contains("data scientist"));
        assert!(result.contains("deadline"));
        assert!(!result.contains("⚠"));
    }

    #[test]
    fn read_and_truncate_exceeds_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("MEMORY.md");
        let lines: Vec<String> = (0..250).map(|i| format!("- line {i}")).collect();
        fs::write(&path, lines.join("\n")).unwrap();
        let result = read_and_truncate(&path).unwrap();
        // Should contain first 200 lines.
        assert!(result.contains("- line 0"));
        assert!(result.contains("- line 199"));
        // Should NOT contain line 200+.
        assert!(!result.contains("- line 200\n"));
        // Should have truncation warning.
        assert!(result.contains("250 lines (limit: 200)"));
    }

    #[test]
    fn read_and_truncate_exceeds_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("MEMORY.md");
        // 180 lines, each ~200 bytes → ~36KB > 25KB limit.
        let lines: Vec<String> = (0..180)
            .map(|i| format!("- line {i}: {}", "x".repeat(190)))
            .collect();
        fs::write(&path, lines.join("\n")).unwrap();
        let result = read_and_truncate(&path).unwrap();
        assert!(result.len() <= MAX_MEMORY_BYTES + 200); // allow room for warning
        assert!(result.contains("⚠"));
        assert!(result.contains("bytes (limit: 25000)"));
    }

    #[test]
    fn read_and_truncate_file_not_found() {
        let result = read_and_truncate(Path::new("/nonexistent/MEMORY.md"));
        assert!(result.is_none());
    }

    #[test]
    fn read_and_truncate_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("MEMORY.md");
        fs::write(&path, "").unwrap();
        let result = read_and_truncate(&path).unwrap();
        assert!(result.is_empty());
    }

    // ── is_memory_path tests ──

    #[test]
    fn is_memory_path_matches() {
        assert!(is_memory_path(".krew/memory/MEMORY.md"));
        assert!(is_memory_path(".krew/memory/user_role.md"));
        assert!(is_memory_path(".krew/memory/agents/gpt/MEMORY.md"));
        assert!(is_memory_path(".krew/memory/agents/opus/feedback.md"));
    }

    #[test]
    fn is_memory_path_not_matches() {
        assert!(!is_memory_path(".krew/settings.toml"));
        assert!(!is_memory_path(".krew/sessions/foo.toml"));
        assert!(!is_memory_path("src/memory.rs"));
        assert!(!is_memory_path(".git/config"));
    }

    #[test]
    fn is_memory_path_case_insensitive() {
        assert!(is_memory_path(".Krew/Memory/MEMORY.md"));
        assert!(is_memory_path(".KREW/MEMORY/user.md"));
    }

    // ── load_memory_prompt tests ──

    #[test]
    fn load_memory_prompt_no_memory_dir() {
        // Non-writable path should fail silently.
        // Use a path that likely doesn't exist and can't be created.
        // On most systems this will fail at create_dir_all.
        let result = load_memory_prompt("test", "/nonexistent/path/that/cannot/exist", true);
        // Either None (can't create dir) or Some with just template (created but empty).
        // Both are acceptable.
        if let Some(content) = &result {
            assert!(content.contains("# Memory") || content.is_empty());
        }
    }

    #[test]
    fn load_memory_prompt_only_global() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        let mem_dir = cwd.join(".krew").join("memory");
        fs::create_dir_all(&mem_dir).unwrap();
        fs::create_dir_all(mem_dir.join("agents").join("test")).unwrap();
        fs::write(mem_dir.join("MEMORY.md"), "- [User](user.md) — engineer").unwrap();

        let result = load_memory_prompt("test", cwd.to_str().unwrap(), true).unwrap();
        assert!(result.contains("# Memory"));
        assert!(result.contains("## Global Memory"));
        assert!(result.contains("engineer"));
        assert!(!result.contains("## Your Memory"));
    }

    #[test]
    fn load_memory_prompt_only_agent() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        let agent_dir = cwd.join(".krew").join("memory").join("agents").join("opus");
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(
            agent_dir.join("MEMORY.md"),
            "- [Feedback](fb.md) — no emoji",
        )
        .unwrap();

        let result = load_memory_prompt("opus", cwd.to_str().unwrap(), true).unwrap();
        assert!(result.contains("# Memory"));
        assert!(!result.contains("## Global Memory"));
        assert!(result.contains("## Your Memory"));
        assert!(result.contains("no emoji"));
    }

    #[test]
    fn load_memory_prompt_both() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        let mem_dir = cwd.join(".krew").join("memory");
        let agent_dir = mem_dir.join("agents").join("gpt");
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(mem_dir.join("MEMORY.md"), "- [User](user.md) — PM").unwrap();
        fs::write(agent_dir.join("MEMORY.md"), "- [Style](style.md) — concise").unwrap();

        let result = load_memory_prompt("gpt", cwd.to_str().unwrap(), true).unwrap();
        assert!(result.contains("## Global Memory"));
        assert!(result.contains("PM"));
        assert!(result.contains("## Your Memory"));
        assert!(result.contains("concise"));
    }

    #[test]
    fn load_memory_prompt_empty_files() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        let mem_dir = cwd.join(".krew").join("memory");
        let agent_dir = mem_dir.join("agents").join("test");
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(mem_dir.join("MEMORY.md"), "").unwrap();
        fs::write(agent_dir.join("MEMORY.md"), "").unwrap();

        let result = load_memory_prompt("test", cwd.to_str().unwrap(), true).unwrap();
        // Only template, no Global/Your sections.
        assert!(result.contains("# Memory"));
        assert!(!result.contains("## Global Memory"));
        assert!(!result.contains("## Your Memory"));
    }

    #[test]
    fn load_memory_prompt_tools_false() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        let mem_dir = cwd.join(".krew").join("memory");
        let agent_dir = mem_dir.join("agents").join("reader");
        fs::create_dir_all(&agent_dir).unwrap();
        fs::write(mem_dir.join("MEMORY.md"), "- [User](user.md) — dev").unwrap();
        fs::write(agent_dir.join("MEMORY.md"), "- [FB](fb.md) — terse").unwrap();

        let result = load_memory_prompt("reader", cwd.to_str().unwrap(), false).unwrap();
        // Should NOT contain write instructions.
        assert!(!result.contains("# Memory"));
        assert!(!result.contains("How to save"));
        // Should contain index content.
        assert!(result.contains("## Global Memory"));
        assert!(result.contains("dev"));
        assert!(result.contains("## Your Memory"));
        assert!(result.contains("terse"));
    }

    #[test]
    fn load_memory_prompt_tools_false_no_content() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        fs::create_dir_all(cwd.join(".krew").join("memory").join("agents").join("r")).unwrap();

        let result = load_memory_prompt("r", cwd.to_str().unwrap(), false);
        // No MEMORY.md files, no template → None.
        assert!(result.is_none());
    }

    #[test]
    fn load_memory_prompt_variable_substitution() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path();
        fs::create_dir_all(
            cwd.join(".krew")
                .join("memory")
                .join("agents")
                .join("mybot"),
        )
        .unwrap();

        let result = load_memory_prompt("mybot", cwd.to_str().unwrap(), true).unwrap();
        assert!(result.contains(".krew/memory/agents/mybot/"));
        assert!(!result.contains("{{agent_name}}"));
    }
}
