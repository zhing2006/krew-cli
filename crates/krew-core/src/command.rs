/// Slash commands available during a session.
pub enum SlashCommand {
    /// Resume a previous session.
    Resume,
    /// List agents and their token usage.
    Agents,
    /// Clear the terminal screen (also aliased as /new).
    Clear,
    /// Compact session context using the specified agent.
    Compact(String),
    /// List MCP servers and tools.
    Mcp,
    /// List available skills.
    Skills,
    /// Show available commands.
    Help,
    /// Exit the program (also aliased as /quit).
    Exit,
}

impl SlashCommand {
    /// Parse a slash command from user input.
    pub fn from_input(input: &str) -> Option<SlashCommand> {
        let input = input.trim();
        let (cmd, arg) = match input.split_once(' ') {
            Some((c, a)) => (c, a.trim().to_string()),
            None => (input, String::new()),
        };
        match cmd {
            "/clear" | "/new" => Some(SlashCommand::Clear),
            "/resume" => Some(SlashCommand::Resume),
            "/agents" => Some(SlashCommand::Agents),
            "/compact" => Some(SlashCommand::Compact(arg)),
            "/mcp" => Some(SlashCommand::Mcp),
            "/skills" => Some(SlashCommand::Skills),
            "/help" => Some(SlashCommand::Help),
            "/exit" | "/quit" => Some(SlashCommand::Exit),
            _ => None,
        }
    }

    /// Return the command name including the `/` prefix.
    pub fn name(&self) -> &str {
        match self {
            SlashCommand::Clear => "/clear",
            SlashCommand::Resume => "/resume",
            SlashCommand::Agents => "/agents",
            SlashCommand::Compact(_) => "/compact",
            SlashCommand::Mcp => "/mcp",
            SlashCommand::Skills => "/skills",
            SlashCommand::Help => "/help",
            SlashCommand::Exit => "/exit",
        }
    }

    /// Return all command names and descriptions (for `/help` listing).
    pub fn all_help() -> &'static [(&'static str, &'static str)] {
        &[
            ("/clear", "Clear the screen (/new)"),
            ("/resume", "Resume a previous session"),
            ("/agents", "List agents and token usage"),
            ("/compact", "Compact session context"),
            ("/mcp", "List MCP servers and tools"),
            ("/skills", "List available skills"),
            ("/help", "Show available commands"),
            ("/exit", "Exit the program (/quit)"),
        ]
    }

    /// Return a short description of the command.
    pub fn description(&self) -> &str {
        match self {
            SlashCommand::Clear => "Clear the screen (/new)",
            SlashCommand::Resume => "Resume a previous session",
            SlashCommand::Agents => "List agents and token usage",
            SlashCommand::Compact(_) => "Compact session context",
            SlashCommand::Mcp => "List MCP servers and tools",
            SlashCommand::Skills => "List available skills",
            SlashCommand::Help => "Show available commands",
            SlashCommand::Exit => "Exit the program (/quit)",
        }
    }
}
