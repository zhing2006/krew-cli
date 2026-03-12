//! Parse a custom command `.md` file into frontmatter + body.

use super::CustomCommand;

/// Parse a command file's content into a `CustomCommand`.
///
/// Expected format:
/// ```text
/// ---
/// description: Some description
/// argument-hint: [args]
/// ---
/// Body content here
/// ```
///
/// Frontmatter is optional. If absent, the entire content is the body.
pub fn parse_command_file(name: &str, content: &str) -> CustomCommand {
    let (description, argument_hint, body) = parse_frontmatter(content);
    CustomCommand {
        name: name.to_string(),
        description,
        argument_hint,
        body,
    }
}

/// Split content into (description, argument_hint, body).
fn parse_frontmatter(content: &str) -> (String, String, String) {
    let trimmed = content.trim_start();

    // Check for opening `---`.
    if !trimmed.starts_with("---") {
        return (String::new(), String::new(), content.to_string());
    }

    // Find the closing `---` after the first line.
    let after_open = &trimmed[3..];
    let after_open = after_open.strip_prefix('\n').unwrap_or(after_open);

    let Some(close_pos) = after_open.find("\n---") else {
        // No closing delimiter — treat entire content as body.
        return (String::new(), String::new(), content.to_string());
    };

    let frontmatter_block = &after_open[..close_pos];
    let body_start = close_pos + 4; // "\n---".len()
    let body = after_open[body_start..]
        .strip_prefix('\n')
        .unwrap_or(&after_open[body_start..]);

    let mut description = String::new();
    let mut argument_hint = String::new();

    for line in frontmatter_block.lines() {
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim().to_string();
            match key {
                "description" => description = value,
                "argument-hint" => argument_hint = value,
                _ => {} // Ignore unknown fields.
            }
        }
    }

    (description, argument_hint, body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_frontmatter() {
        let content = "---\ndescription: Create a commit\nargument-hint: [message]\n---\n@coder commit: $ARGUMENTS\n";
        let cmd = parse_command_file("commit", content);
        assert_eq!(cmd.name, "commit");
        assert_eq!(cmd.description, "Create a commit");
        assert_eq!(cmd.argument_hint, "[message]");
        assert_eq!(cmd.body, "@coder commit: $ARGUMENTS\n");
    }

    #[test]
    fn test_partial_frontmatter() {
        let content = "---\ndescription: Review code\n---\n@reviewer please review\n";
        let cmd = parse_command_file("review", content);
        assert_eq!(cmd.description, "Review code");
        assert_eq!(cmd.argument_hint, "");
        assert_eq!(cmd.body, "@reviewer please review\n");
    }

    #[test]
    fn test_no_frontmatter() {
        let content = "@coder hello world\n";
        let cmd = parse_command_file("hello", content);
        assert_eq!(cmd.description, "");
        assert_eq!(cmd.argument_hint, "");
        assert_eq!(cmd.body, "@coder hello world\n");
    }

    #[test]
    fn test_empty_file() {
        let cmd = parse_command_file("empty", "");
        assert_eq!(cmd.description, "");
        assert_eq!(cmd.argument_hint, "");
        assert_eq!(cmd.body, "");
    }
}
