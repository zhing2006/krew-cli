//! Integration tests for custom command discovery, parsing, argument
//! substitution, and routing.

use std::fs;

use krew_core::custom_command::discovery::discover_commands;
use krew_core::router;

/// Create a test fixture with sample command files.
fn create_fixtures(base: &std::path::Path) {
    let cmd_dir = base.join(".krew").join("commands");

    // Flat command.
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(
        cmd_dir.join("commit.md"),
        "---\ndescription: Create a git commit\nargument-hint: [message]\n---\n@coder Write a commit message for: $ARGUMENTS\n",
    )
    .unwrap();

    // Nested command.
    let review_dir = cmd_dir.join("review");
    fs::create_dir_all(&review_dir).unwrap();
    fs::write(
        review_dir.join("pr.md"),
        "---\ndescription: Review a PR\n---\n@reviewer Please review PR #$1\n",
    )
    .unwrap();

    // Command without frontmatter.
    fs::write(cmd_dir.join("hello.md"), "@all Hello everyone!\n").unwrap();
}

#[test]
fn test_discovery_and_expansion() {
    let tmp = tempfile::tempdir().unwrap();
    create_fixtures(tmp.path());

    let registry = discover_commands(tmp.path());

    // All 3 commands discovered.
    assert_eq!(registry.list().len(), 3);

    // Flat command.
    let commit = registry.lookup("commit").unwrap();
    assert_eq!(commit.description, "Create a git commit");
    assert_eq!(commit.argument_hint, "[message]");

    // Namespace command.
    let review_pr = registry.lookup("review:pr").unwrap();
    assert_eq!(review_pr.description, "Review a PR");

    // No-frontmatter command.
    let hello = registry.lookup("hello").unwrap();
    assert_eq!(hello.description, "");
}

#[test]
fn test_argument_substitution_and_routing() {
    let tmp = tempfile::tempdir().unwrap();
    create_fixtures(tmp.path());

    let registry = discover_commands(tmp.path());

    // Test $ARGUMENTS substitution.
    let commit = registry.lookup("commit").unwrap();
    let expanded = commit.substitute_args("fix typo in readme");
    assert!(expanded.contains("fix typo in readme"));
    assert!(!expanded.contains("$ARGUMENTS"));

    // Test positional $1 substitution.
    let review = registry.lookup("review:pr").unwrap();
    let expanded = review.substitute_args("42");
    assert!(expanded.contains("#42"));
    assert!(!expanded.contains("$1"));

    // Test routing: expanded text should route to the correct agent.
    let agents = vec!["coder".to_string(), "reviewer".to_string()];
    let (addressee, _body) =
        router::parse_input(commit.substitute_args("fix").trim(), &agents).unwrap();
    assert!(matches!(addressee, router::Addressee::Single(ref name) if name == "coder"));

    let (addressee, _body) =
        router::parse_input(review.substitute_args("42").trim(), &agents).unwrap();
    assert!(matches!(addressee, router::Addressee::Single(ref name) if name == "reviewer"));

    // @all routing.
    let hello = registry.lookup("hello").unwrap();
    let expanded = hello.substitute_args("");
    let (addressee, _body) = router::parse_input(expanded.trim(), &agents).unwrap();
    assert!(matches!(addressee, router::Addressee::All));
}
