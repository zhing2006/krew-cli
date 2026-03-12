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
