/// Scope for the `/dream` memory consolidation command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DreamScope {
    /// Consolidate global memory (`.krew/memory/`).
    Global,
    /// Consolidate per-agent memory (`.krew/memory/agents/{name}/`).
    Agent,
    /// Consolidate both global and per-agent memory.
    All,
}

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
    /// List available tools per agent.
    Tools,
    /// Show process stats (memory, threads).
    Stats,
    /// Show available commands.
    Help,
    /// Rewind to a previous message.
    Rewind,
    /// Memory consolidation (dream) with a specific agent.
    Dream(DreamScope, String),
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
            "/tools" => Some(SlashCommand::Tools),
            "/stats" => Some(SlashCommand::Stats),
            "/help" => Some(SlashCommand::Help),
            "/rewind" => Some(SlashCommand::Rewind),
            "/dream" => Self::parse_dream(&arg),
            "/exit" | "/quit" => Some(SlashCommand::Exit),
            _ => None,
        }
    }

    /// Parse `/dream <scope> @<agent>` arguments.
    ///
    /// Returns `Some(Dream(scope, agent))` on success, or
    /// `Some(Dream(Global, ""))` for missing/invalid args (handled at
    /// execution time with a usage hint).
    fn parse_dream(arg: &str) -> Option<SlashCommand> {
        if arg.is_empty() {
            // No arguments — show usage hint at execution time.
            return Some(SlashCommand::Dream(DreamScope::Global, String::new()));
        }

        let mut parts = arg.split_whitespace();
        let scope_str = parts.next().unwrap_or("");
        let scope = match scope_str {
            "global" => DreamScope::Global,
            "agent" => DreamScope::Agent,
            "all" => DreamScope::All,
            _ => {
                // Invalid scope — show usage hint at execution time.
                return Some(SlashCommand::Dream(DreamScope::Global, String::new()));
            }
        };

        let agent_part = parts.next().unwrap_or("");
        let agent_name = agent_part.strip_prefix('@').unwrap_or("").to_string();

        Some(SlashCommand::Dream(scope, agent_name))
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
            SlashCommand::Tools => "/tools",
            SlashCommand::Stats => "/stats",
            SlashCommand::Help => "/help",
            SlashCommand::Rewind => "/rewind",
            SlashCommand::Dream(..) => "/dream",
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
            ("/dream", "Consolidate memory (/dream <scope> @<agent>)"),
            ("/mcp", "List MCP servers and tools"),
            ("/skills", "List available skills"),
            ("/tools", "List available tools per agent"),
            ("/stats", "Show process stats (memory, threads)"),
            ("/help", "Show available commands"),
            ("/rewind", "Rewind to a previous message"),
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
            SlashCommand::Dream(..) => "Consolidate memory (/dream <scope> @<agent>)",
            SlashCommand::Mcp => "List MCP servers and tools",
            SlashCommand::Skills => "List available skills",
            SlashCommand::Tools => "List available tools per agent",
            SlashCommand::Stats => "Show process stats (memory, threads)",
            SlashCommand::Help => "Show available commands",
            SlashCommand::Rewind => "Rewind to a previous message",
            SlashCommand::Exit => "Exit the program (/quit)",
        }
    }
}
