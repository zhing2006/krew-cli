/// Slash commands available during a session.
pub enum SlashCommand {
    /// Start a new session.
    New,
    /// Resume a previous session.
    Resume,
    /// List agents and their token usage.
    Agents,
    /// Clear the terminal screen.
    Clear,
    /// Compact session context using the specified agent.
    Compact(String),
    /// Show available commands.
    Help,
    /// Exit the program.
    Quit,
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
            "/new" => Some(SlashCommand::New),
            "/resume" => Some(SlashCommand::Resume),
            "/agents" => Some(SlashCommand::Agents),
            "/clear" => Some(SlashCommand::Clear),
            "/compact" => Some(SlashCommand::Compact(arg)),
            "/help" => Some(SlashCommand::Help),
            "/quit" => Some(SlashCommand::Quit),
            _ => None,
        }
    }

    /// Return the command name including the `/` prefix.
    pub fn name(&self) -> &str {
        match self {
            SlashCommand::New => "/new",
            SlashCommand::Resume => "/resume",
            SlashCommand::Agents => "/agents",
            SlashCommand::Clear => "/clear",
            SlashCommand::Compact(_) => "/compact",
            SlashCommand::Help => "/help",
            SlashCommand::Quit => "/quit",
        }
    }

    /// Return a short description of the command.
    pub fn description(&self) -> &str {
        match self {
            SlashCommand::New => "Start a new session",
            SlashCommand::Resume => "Resume a previous session",
            SlashCommand::Agents => "List agents and token usage",
            SlashCommand::Clear => "Clear the screen",
            SlashCommand::Compact(_) => "Compact session context",
            SlashCommand::Help => "Show available commands",
            SlashCommand::Quit => "Quit the program",
        }
    }
}
