//! Tests for discovery_paths, multi-directory command discovery,
//! and multi-directory skill discovery.

use std::fs;

use krew_core::custom_command::discovery::discover_commands;
use krew_core::discovery::discovery_paths;
use krew_core::skill::discover_skills;

// ── 5.2: discovery_paths returns 6 paths in correct order ───────────

#[test]
fn discovery_paths_full_list() {
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path();
    let paths = discovery_paths(cwd, "commands");

    // Should have at least 3 project-level paths, plus 3 user-level if HOME is set.
    assert!(paths.len() >= 3);
    assert_eq!(paths[0], cwd.join(".krew").join("commands"));
    assert_eq!(paths[1], cwd.join(".agents").join("commands"));
    assert_eq!(paths[2], cwd.join(".claude").join("commands"));

    // If HOME/USERPROFILE is set, should have 6 paths.
    if std::env::var_os("HOME").is_some() || std::env::var_os("USERPROFILE").is_some() {
        assert_eq!(paths.len(), 6);
        // User-level paths should contain .krew, .agents, .claude in order.
        assert!(paths[3].ends_with(".krew/commands") || paths[3].ends_with(".krew\\commands"));
        assert!(paths[4].ends_with(".agents/commands") || paths[4].ends_with(".agents\\commands"));
        assert!(paths[5].ends_with(".claude/commands") || paths[5].ends_with(".claude\\commands"));
    }
}

// ── 5.4: subdir parameter works for different values ────────────────

#[test]
fn discovery_paths_skills_subdir() {
    let tmp = tempfile::tempdir().unwrap();
    let paths = discovery_paths(tmp.path(), "skills");
    assert_eq!(paths[0], tmp.path().join(".krew").join("skills"));
    assert_eq!(paths[1], tmp.path().join(".agents").join("skills"));
    assert_eq!(paths[2], tmp.path().join(".claude").join("skills"));
}

// ── 6.4: .krew/commands/ normal discovery ───────────────────────────

#[test]
fn commands_krew_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let cmd_dir = tmp.path().join(".krew").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("deploy.md"), "deploy everything\n").unwrap();

    let registry = discover_commands(tmp.path());
    assert!(registry.lookup("deploy").is_some());
}

// ── 6.5: .agents/commands/ discovery ────────────────────────────────

#[test]
fn commands_agents_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let cmd_dir = tmp.path().join(".agents").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("review.md"), "review code\n").unwrap();

    let registry = discover_commands(tmp.path());
    assert!(registry.lookup("review").is_some());
}

// ── 6.6: .claude/commands/ discovery ────────────────────────────────

#[test]
fn commands_claude_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let cmd_dir = tmp.path().join(".claude").join("commands");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(cmd_dir.join("lint.md"), "run linter\n").unwrap();

    let registry = discover_commands(tmp.path());
    assert!(registry.lookup("lint").is_some());
}

// ── 6.7: priority — .krew > .agents > .claude ──────────────────────

#[test]
fn commands_priority_krew_over_agents_over_claude() {
    let tmp = tempfile::tempdir().unwrap();

    // Create same command in all three dirs with different descriptions.
    for (dir_name, desc) in [
        (".krew", "from krew"),
        (".agents", "from agents"),
        (".claude", "from claude"),
    ] {
        let cmd_dir = tmp.path().join(dir_name).join("commands");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(
            cmd_dir.join("deploy.md"),
            format!("---\ndescription: {desc}\n---\ndo deploy\n"),
        )
        .unwrap();
    }

    let registry = discover_commands(tmp.path());
    let cmd = registry.lookup("deploy").unwrap();
    assert_eq!(cmd.description, "from krew");

    // Now test agents > claude (without krew).
    let tmp2 = tempfile::tempdir().unwrap();
    for (dir_name, desc) in [(".agents", "from agents"), (".claude", "from claude")] {
        let cmd_dir = tmp2.path().join(dir_name).join("commands");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(
            cmd_dir.join("deploy.md"),
            format!("---\ndescription: {desc}\n---\ndo deploy\n"),
        )
        .unwrap();
    }

    let registry2 = discover_commands(tmp2.path());
    let cmd2 = registry2.lookup("deploy").unwrap();
    assert_eq!(cmd2.description, "from agents");
}

// ── 6.8: multi-directory merge (different names) ────────────────────

#[test]
fn commands_multi_dir_merge() {
    let tmp = tempfile::tempdir().unwrap();

    let krew_dir = tmp.path().join(".krew").join("commands");
    fs::create_dir_all(&krew_dir).unwrap();
    fs::write(krew_dir.join("commit.md"), "commit\n").unwrap();

    let agents_dir = tmp.path().join(".agents").join("commands");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(agents_dir.join("review.md"), "review\n").unwrap();

    let claude_dir = tmp.path().join(".claude").join("commands");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(claude_dir.join("lint.md"), "lint\n").unwrap();

    let registry = discover_commands(tmp.path());
    assert_eq!(registry.list().len(), 3);
    assert!(registry.lookup("commit").is_some());
    assert!(registry.lookup("review").is_some());
    assert!(registry.lookup("lint").is_some());
}

// ── 6.9: all dirs missing → empty registry ──────────────────────────

#[test]
fn commands_no_dirs_empty_registry() {
    let tmp = tempfile::tempdir().unwrap();
    let registry = discover_commands(tmp.path());
    assert!(registry.is_empty());
}

// ── 7.2: .claude/skills/ discovery ──────────────────────────────────

#[test]
fn skills_claude_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let skill_dir = tmp.path().join(".claude").join("skills").join("my-skill");
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: my-skill\ndescription: A skill from claude dir.\n---\n\nInstructions.",
    )
    .unwrap();

    let skills = discover_skills(tmp.path(), &[]);
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].name, "my-skill");
}

// ── 7.3: skills priority .krew > .agents > .claude ──────────────────

#[test]
fn skills_priority_krew_over_agents_over_claude() {
    let tmp = tempfile::tempdir().unwrap();

    for (dir_name, desc) in [
        (".krew", "From krew."),
        (".agents", "From agents."),
        (".claude", "From claude."),
    ] {
        let skill_dir = tmp.path().join(dir_name).join("skills").join("dupe");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: dupe\ndescription: {desc}\n---\n\nBody."),
        )
        .unwrap();
    }

    let skills = discover_skills(tmp.path(), &[]);
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].description, "From krew.");
}

// ── 7.5: multi-directory skill merge ────────────────────────────────

#[test]
fn skills_multi_dir_merge() {
    let tmp = tempfile::tempdir().unwrap();

    let krew_dir = tmp.path().join(".krew").join("skills").join("review");
    fs::create_dir_all(&krew_dir).unwrap();
    fs::write(
        krew_dir.join("SKILL.md"),
        "---\nname: review\ndescription: Code review.\n---\n\nReview.",
    )
    .unwrap();

    let claude_dir = tmp.path().join(".claude").join("skills").join("search");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join("SKILL.md"),
        "---\nname: search\ndescription: Web search.\n---\n\nSearch.",
    )
    .unwrap();

    let agents_dir = tmp.path().join(".agents").join("skills").join("deploy");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("SKILL.md"),
        "---\nname: deploy\ndescription: Deploy.\n---\n\nDeploy.",
    )
    .unwrap();

    let skills = discover_skills(tmp.path(), &[]);
    assert_eq!(skills.len(), 3);
    let names: Vec<&str> = skills.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"review"));
    assert!(names.contains(&"search"));
    assert!(names.contains(&"deploy"));
}
