/// Target addressee for a user message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Addressee {
    /// Broadcast to all agents (`@all`).
    All,
    /// Direct message to a specific agent (`@name`).
    Single(String),
    /// Direct message to multiple specific agents (`@gpt @opus`).
    Multiple(Vec<String>),
    /// Send to the last agent that responded (no `@` prefix).
    LastRespondent,
}

/// Parse user input into an addressee and message body.
///
/// `@name` tokens are recognized anywhere in the input, but only if `name`
/// matches a known agent or `"all"`. Unrecognized `@tokens` (including
/// bare `@`) are treated as plain text.
///
/// The message body is always the **full original input** — `@name` tokens
/// are not stripped. This preserves context for the LLM.
pub fn parse_input(input: &str, known_agents: &[String]) -> anyhow::Result<(Addressee, String)> {
    let input = input.trim();
    if input.is_empty() {
        anyhow::bail!("empty input");
    }

    // Scan for @name tokens matching known agents (or "all").
    let mut matched: Vec<String> = Vec::new();
    for word in input.split_whitespace() {
        if let Some(name) = word.strip_prefix('@')
            && (name == "all" || known_agents.iter().any(|a| a == name))
            && !matched.contains(&name.to_string())
        {
            matched.push(name.to_string());
        }
    }

    let message = input.to_string();

    if matched.is_empty() {
        Ok((Addressee::LastRespondent, message))
    } else if matched.iter().any(|n| n == "all") {
        // @all takes priority — even if mixed with specific names.
        Ok((Addressee::All, message))
    } else if matched.len() == 1 {
        Ok((
            Addressee::Single(matched.into_iter().next().unwrap()),
            message,
        ))
    } else {
        Ok((Addressee::Multiple(matched), message))
    }
}
