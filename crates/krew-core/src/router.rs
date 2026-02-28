/// Target addressee for a user message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Addressee {
    /// Broadcast to all agents (`@all`).
    All,
    /// Direct message to a specific agent (`@name`).
    Single(String),
    /// Send to the last agent that responded (no `@` prefix).
    LastRespondent,
}

/// Parse user input into an addressee and message body.
pub fn parse_input(input: &str) -> anyhow::Result<(Addressee, String)> {
    let input = input.trim();
    if input.is_empty() {
        anyhow::bail!("empty input");
    }

    if input == "@all" || input.starts_with("@all ") {
        let message = input.strip_prefix("@all").unwrap().trim_start().to_string();
        Ok((Addressee::All, message))
    } else if let Some(at_rest) = input.strip_prefix('@') {
        let (name, message) = match at_rest.split_once(' ') {
            Some((n, m)) => (n.to_string(), m.to_string()),
            None => (at_rest.to_string(), String::new()),
        };
        if name.is_empty() {
            anyhow::bail!("agent name required after @");
        }
        Ok((Addressee::Single(name), message))
    } else {
        Ok((Addressee::LastRespondent, input.to_string()))
    }
}
