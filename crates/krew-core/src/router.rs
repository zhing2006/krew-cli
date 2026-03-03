use std::collections::{HashSet, VecDeque};

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

/// Resolve an `Addressee` into an ordered queue of agent names to dispatch.
///
/// - `All` uses `reply_order`, filtered to agents with active LLM clients.
/// - `Multiple` preserves the `@` appearance order.
/// - `Single` and `LastRespondent` produce a single-element queue.
pub fn resolve_dispatch_queue(
    addressee: &Addressee,
    reply_order: &[String],
    available_agents: &HashSet<String>,
    last_respondent: Option<&str>,
) -> VecDeque<String> {
    let mut queue = VecDeque::new();

    match addressee {
        Addressee::All => {
            for name in reply_order {
                if available_agents.contains(name) {
                    queue.push_back(name.clone());
                }
            }
        }
        Addressee::Multiple(names) => {
            for name in names {
                if available_agents.contains(name) {
                    queue.push_back(name.clone());
                }
            }
        }
        Addressee::Single(name) => {
            queue.push_back(name.clone());
        }
        Addressee::LastRespondent => {
            if let Some(name) = last_respondent {
                queue.push_back(name.to_string());
            }
        }
    }

    queue
}

/// Resolve the list of target agent names for display purposes (e.g. colored
/// routing dots in the TUI).
pub fn resolve_target_names<'a>(
    addressee: &'a Addressee,
    reply_order: &'a [String],
    available_agents: &HashSet<String>,
    last_respondent: Option<&'a str>,
) -> Vec<&'a str> {
    match addressee {
        Addressee::All => reply_order
            .iter()
            .filter(|name| available_agents.contains(name.as_str()))
            .map(|n| n.as_str())
            .collect(),
        Addressee::Single(name) => vec![name.as_str()],
        Addressee::Multiple(names) => names.iter().map(|n| n.as_str()).collect(),
        Addressee::LastRespondent => last_respondent.map(|n| vec![n]).unwrap_or_default(),
    }
}
