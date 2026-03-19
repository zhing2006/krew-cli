//! Custom slash commands loaded from `.krew/commands/` directory.

pub mod discovery;
pub mod parser;
pub mod preprocessor;

use std::collections::HashMap;

/// A custom slash command parsed from a `.md` file.
#[derive(Debug, Clone)]
pub struct CustomCommand {
    /// Command name (e.g. "commit" or "review:pr").
    pub name: String,
    /// Description from frontmatter (may be empty).
    pub description: String,
    /// Argument hint from frontmatter (may be empty).
    pub argument_hint: String,
    /// Command body (markdown content after frontmatter).
    pub body: String,
}

impl CustomCommand {
    /// Substitute argument placeholders in the command body.
    ///
    /// - `$ARGUMENTS` → full argument string
    /// - `$1`, `$2`, ... → positional arguments (whitespace-split)
    /// - Unmatched positional placeholders → empty string
    pub fn substitute_args(&self, args: &str) -> String {
        let positional: Vec<&str> = if args.is_empty() {
            Vec::new()
        } else {
            args.split_whitespace().collect()
        };

        let mut result = self.body.replace("$ARGUMENTS", args);

        // Replace positional args $1..$9 (support up to 9).
        for i in (1..=9).rev() {
            let placeholder = format!("${i}");
            let value = positional.get(i - 1).copied().unwrap_or("");
            result = result.replace(&placeholder, value);
        }

        result
    }

    /// Expand the command: extract `@agent` tokens from args for routing,
    /// substitute remaining args into the body, and prepend `@` tokens.
    ///
    /// If no `@agent` in args, the result has no routing prefix
    /// (downstream `parse_input` defaults to `LastRespondent`).
    pub fn expand(&self, args: &str) -> String {
        let (at_tokens, rest_args) = split_addressee(args);
        let body = self.substitute_args(&rest_args);

        if at_tokens.is_empty() {
            body
        } else {
            let prefix = at_tokens.join(" ");
            format!("{prefix} {body}")
        }
    }
}

/// Separate leading `@xxx` tokens from the rest of the argument string.
///
/// Returns `(vec_of_at_tokens, remaining_args)`.
/// Only tokens at the **beginning** of `args` that start with `@` are extracted;
/// once a non-`@` token is encountered, everything from that point onward is
/// kept as remaining args (preserving original spacing).
fn split_addressee(args: &str) -> (Vec<&str>, String) {
    let trimmed = args.trim();
    if trimmed.is_empty() {
        return (Vec::new(), String::new());
    }

    let mut at_tokens = Vec::new();
    let mut remaining_start = 0;

    for token in trimmed.split_whitespace() {
        if token.starts_with('@') {
            at_tokens.push(token);
            // Advance past this token in the original trimmed string.
            let token_pos = trimmed[remaining_start..].find(token).unwrap();
            remaining_start += token_pos + token.len();
        } else {
            break;
        }
    }

    let rest = trimmed[remaining_start..].trim_start().to_string();
    (at_tokens, rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cmd(body: &str) -> CustomCommand {
        CustomCommand {
            name: "test".to_string(),
            description: String::new(),
            argument_hint: String::new(),
            body: body.to_string(),
        }
    }

    #[test]
    fn split_addressee_no_at() {
        let (at, rest) = split_addressee("file.rs");
        assert!(at.is_empty());
        assert_eq!(rest, "file.rs");
    }

    #[test]
    fn split_addressee_single_at() {
        let (at, rest) = split_addressee("@coder file.rs");
        assert_eq!(at, vec!["@coder"]);
        assert_eq!(rest, "file.rs");
    }

    #[test]
    fn split_addressee_multiple_at() {
        let (at, rest) = split_addressee("@coder @reviewer file.rs main.rs");
        assert_eq!(at, vec!["@coder", "@reviewer"]);
        assert_eq!(rest, "file.rs main.rs");
    }

    #[test]
    fn split_addressee_only_at() {
        let (at, rest) = split_addressee("@coder");
        assert_eq!(at, vec!["@coder"]);
        assert_eq!(rest, "");
    }

    #[test]
    fn split_addressee_empty() {
        let (at, rest) = split_addressee("");
        assert!(at.is_empty());
        assert_eq!(rest, "");
    }

    #[test]
    fn expand_no_at_routes_last_respondent() {
        let cmd = make_cmd("Review $ARGUMENTS");
        let result = cmd.expand("file.rs");
        assert_eq!(result, "Review file.rs");
        // No @ prefix → parse_input will return LastRespondent.
        assert!(!result.starts_with('@'));
    }

    #[test]
    fn expand_with_single_at() {
        let cmd = make_cmd("Review $ARGUMENTS");
        let result = cmd.expand("@coder file.rs");
        assert_eq!(result, "@coder Review file.rs");
    }

    #[test]
    fn expand_with_multiple_at() {
        let cmd = make_cmd("Review $ARGUMENTS");
        let result = cmd.expand("@coder @reviewer file.rs");
        assert_eq!(result, "@coder @reviewer Review file.rs");
    }

    #[test]
    fn expand_empty_args() {
        let cmd = make_cmd("Do something with $ARGUMENTS");
        let result = cmd.expand("");
        assert_eq!(result, "Do something with ");
        assert!(!result.starts_with('@'));
    }

    #[test]
    fn expand_positional_with_at() {
        let cmd = make_cmd("Check $1 against $2");
        let result = cmd.expand("@coder foo.rs bar.rs");
        assert_eq!(result, "@coder Check foo.rs against bar.rs");
    }
}

/// Registry of custom commands, keyed by command name.
#[derive(Debug, Default)]
pub struct CustomCommandRegistry {
    commands: HashMap<String, CustomCommand>,
}

impl CustomCommandRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Insert a command into the registry.
    pub fn insert(&mut self, cmd: CustomCommand) {
        self.commands.insert(cmd.name.clone(), cmd);
    }

    /// Look up a command by name.
    pub fn lookup(&self, name: &str) -> Option<&CustomCommand> {
        self.commands.get(name)
    }

    /// Return all commands sorted by name.
    pub fn list(&self) -> Vec<&CustomCommand> {
        let mut cmds: Vec<&CustomCommand> = self.commands.values().collect();
        cmds.sort_by(|a, b| a.name.cmp(&b.name));
        cmds
    }

    /// Returns true if the registry has no commands.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
