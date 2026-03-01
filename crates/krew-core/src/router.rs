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
    } else if matched.len() == 1 && matched[0] == "all" {
        Ok((Addressee::All, message))
    } else {
        // Filter out "all" if mixed with specific names.
        let agents: Vec<String> = matched.into_iter().filter(|n| n != "all").collect();
        if agents.len() == 1 {
            Ok((
                Addressee::Single(agents.into_iter().next().unwrap()),
                message,
            ))
        } else {
            Ok((Addressee::Multiple(agents), message))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agents() -> Vec<String> {
        vec!["gpt".to_string(), "opus".to_string()]
    }

    #[test]
    fn parse_all() {
        let (addr, msg) = parse_input("@all hello", &agents()).unwrap();
        assert_eq!(addr, Addressee::All);
        assert_eq!(msg, "@all hello");
    }

    #[test]
    fn parse_single_at_start() {
        let (addr, msg) = parse_input("@gpt explain this", &agents()).unwrap();
        assert_eq!(addr, Addressee::Single("gpt".to_string()));
        assert_eq!(msg, "@gpt explain this");
    }

    #[test]
    fn parse_single_in_middle() {
        let (addr, msg) = parse_input("hey @gpt what do you think", &agents()).unwrap();
        assert_eq!(addr, Addressee::Single("gpt".to_string()));
        assert_eq!(msg, "hey @gpt what do you think");
    }

    #[test]
    fn parse_single_at_end() {
        let (addr, msg) = parse_input("explain this @gpt", &agents()).unwrap();
        assert_eq!(addr, Addressee::Single("gpt".to_string()));
        assert_eq!(msg, "explain this @gpt");
    }

    #[test]
    fn parse_multiple() {
        let (addr, msg) = parse_input("@gpt @opus debate this", &agents()).unwrap();
        assert_eq!(
            addr,
            Addressee::Multiple(vec!["gpt".to_string(), "opus".to_string()])
        );
        assert_eq!(msg, "@gpt @opus debate this");
    }

    #[test]
    fn parse_multiple_scattered() {
        let (addr, msg) = parse_input("hey @gpt what does @opus think", &agents()).unwrap();
        assert_eq!(
            addr,
            Addressee::Multiple(vec!["gpt".to_string(), "opus".to_string()])
        );
        assert_eq!(msg, "hey @gpt what does @opus think");
    }

    #[test]
    fn parse_duplicate_deduped() {
        let (addr, msg) = parse_input("@gpt hello @gpt again", &agents()).unwrap();
        assert_eq!(addr, Addressee::Single("gpt".to_string()));
        assert_eq!(msg, "@gpt hello @gpt again");
    }

    #[test]
    fn parse_unknown_agent_is_plain_text() {
        let (addr, msg) = parse_input("@unknown hello", &agents()).unwrap();
        assert_eq!(addr, Addressee::LastRespondent);
        assert_eq!(msg, "@unknown hello");
    }

    #[test]
    fn parse_bare_at_is_plain_text() {
        let (addr, msg) = parse_input("@ hello", &agents()).unwrap();
        assert_eq!(addr, Addressee::LastRespondent);
        assert_eq!(msg, "@ hello");
    }

    #[test]
    fn parse_mixed_known_and_unknown() {
        let (addr, msg) = parse_input("@gpt @unknown hello", &agents()).unwrap();
        assert_eq!(addr, Addressee::Single("gpt".to_string()));
        assert_eq!(msg, "@gpt @unknown hello");
    }

    #[test]
    fn parse_no_prefix() {
        let (addr, msg) = parse_input("just chatting", &agents()).unwrap();
        assert_eq!(addr, Addressee::LastRespondent);
        assert_eq!(msg, "just chatting");
    }

    #[test]
    fn parse_empty_fails() {
        assert!(parse_input("", &agents()).is_err());
    }
}
