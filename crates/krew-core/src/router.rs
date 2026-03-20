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

/// Parse an agent's response text for `@agent_name` mentions.
///
/// Scans whitespace-delimited tokens for `@name` patterns, strips trailing
/// punctuation, and matches against `known_agents`. Self-mentions and `@all`
/// are excluded. Returns matched names in text appearance order (no duplicates).
pub fn parse_agent_mentions(text: &str, known_agents: &[String], self_name: &str) -> Vec<String> {
    let mut matched: Vec<String> = Vec::new();
    for word in text.split_whitespace() {
        if let Some(raw_name) = word.strip_prefix('@') {
            if raw_name.is_empty() {
                continue;
            }
            // Longest-prefix matching against known agents. This handles
            // trailing punctuation ("@opus,"), CJK runs without spaces
            // ("@助手，你觉得呢"), and overlapping names ("foo" vs "foo-bar")
            // by always picking the longest matching agent name.
            let found = known_agents
                .iter()
                .filter(|a| {
                    if raw_name == a.as_str() {
                        return true;
                    }
                    if let Some(rest) = raw_name.strip_prefix(a.as_str()) {
                        rest.starts_with(|c: char| !c.is_alphanumeric())
                    } else {
                        false
                    }
                })
                .max_by_key(|a| a.len());
            if let Some(agent) = found {
                let name = agent.as_str();
                if name == "all" || name == self_name {
                    continue;
                }
                if !matched.contains(agent) {
                    matched.push(agent.clone());
                }
            }
        }
    }
    matched
}

/// Apply the "immediate" AI-to-AI routing strategy to the pending queue.
///
/// If the target is already in the queue, move it to the front.
/// If not, insert it at the front.
pub fn apply_immediate_routing(pending: &mut VecDeque<String>, target: &str) {
    if let Some(pos) = pending.iter().position(|n| n == target) {
        if pos != 0 {
            pending.remove(pos);
            pending.push_front(target.to_string());
        }
    } else {
        pending.push_front(target.to_string());
    }
}

/// Apply the "queued" AI-to-AI routing strategy to the pending queue.
///
/// If the target is not in the queue, append it to the back.
/// If already present, do nothing.
pub fn apply_queued_routing(pending: &mut VecDeque<String>, target: &str) {
    if !pending.iter().any(|n| n == target) {
        pending.push_back(target.to_string());
    }
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
