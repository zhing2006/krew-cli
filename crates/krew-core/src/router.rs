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

/// Parse user input into an addressee, message body, and whisper flag.
///
/// `@name` and `#name` tokens are recognized anywhere in the input, but only
/// if `name` matches a known agent or `"all"`. Unrecognized tokens (including
/// bare `@`/`#`) are treated as plain text.
///
/// `#name` indicates a whisper (private message). `#all` is rejected as an
/// error. Mixing `@` and `#` addressing in the same input is rejected.
///
/// The message body is always the **full original input** — `@name`/`#name`
/// tokens are not stripped. This preserves context for the LLM.
pub fn parse_input(
    input: &str,
    known_agents: &[String],
) -> anyhow::Result<(Addressee, String, bool)> {
    let input = input.trim();
    if input.is_empty() {
        anyhow::bail!("empty input");
    }

    // Scan for @name and #name tokens matching known agents (or "all").
    let mut at_matched: Vec<String> = Vec::new();
    let mut hash_matched: Vec<String> = Vec::new();
    for word in input.split_whitespace() {
        if let Some(name) = word.strip_prefix('@')
            && (name == "all" || known_agents.iter().any(|a| a == name))
            && !at_matched.contains(&name.to_string())
        {
            at_matched.push(name.to_string());
        } else if let Some(name) = word.strip_prefix('#')
            && (name == "all" || known_agents.iter().any(|a| a == name))
            && !hash_matched.contains(&name.to_string())
        {
            hash_matched.push(name.to_string());
        }
    }

    // Reject mixing @ and # addressing.
    if !at_matched.is_empty() && !hash_matched.is_empty() {
        anyhow::bail!("不能同时使用 @ 和 # 寻址");
    }

    // Reject #all.
    if hash_matched.iter().any(|n| n == "all") {
        anyhow::bail!("#all 没有意义——对所有 agent 密语等同于普通消息，请直接发送或使用 @all");
    }

    let message = input.to_string();
    let is_whisper = !hash_matched.is_empty();
    let matched = if is_whisper { hash_matched } else { at_matched };

    if matched.is_empty() {
        Ok((Addressee::LastRespondent, message, false))
    } else if matched.iter().any(|n| n == "all") {
        // @all takes priority — even if mixed with specific names.
        Ok((Addressee::All, message, false))
    } else if matched.len() == 1 {
        Ok((
            Addressee::Single(matched.into_iter().next().unwrap()),
            message,
            is_whisper,
        ))
    } else {
        Ok((Addressee::Multiple(matched), message, is_whisper))
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
    // Scan for every '@' in the text (not just whitespace-delimited tokens),
    // so that CJK punctuation like "太好了！@gemini" is handled correctly.
    for (at_pos, _) in text.match_indices('@') {
        let after = &text[at_pos + 1..];
        if after.is_empty() {
            continue;
        }
        // Longest-prefix matching against known agents.
        let found = known_agents
            .iter()
            .filter(|a| {
                if after == a.as_str() {
                    return true;
                }
                if let Some(rest) = after.strip_prefix(a.as_str()) {
                    // Only ASCII alphanumeric chars extend the name token;
                    // CJK characters act as delimiters (e.g. "@opus看看" matches "opus").
                    rest.starts_with(|c: char| !c.is_ascii_alphanumeric())
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
            // Skip if '@' is preceded by an ASCII alphanumeric character (e.g. "email@agent").
            // CJK characters before '@' are fine (e.g. "问问@opus").
            if at_pos > 0 {
                let prev = text[..at_pos].chars().last().unwrap();
                if prev.is_ascii_alphanumeric() {
                    continue;
                }
            }
            if !matched.contains(agent) {
                matched.push(agent.clone());
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

/// Apply the "immediate" AI-to-AI routing strategy using a cursor position.
///
/// New a2a targets are inserted at `cursor` (not at position 0), so they
/// don't jump ahead of earlier a2a entries. After insertion, `cursor` is
/// incremented. If the target already exists in the queue, it is moved to
/// the cursor position instead.
pub fn apply_immediate_routing_at(
    pending: &mut VecDeque<String>,
    target: &str,
    cursor: &mut usize,
) {
    // Clamp cursor to queue length.
    let pos = (*cursor).min(pending.len());
    if let Some(existing) = pending.iter().position(|n| n == target) {
        if existing != pos {
            pending.remove(existing);
            // Adjust insertion point if the removed element was before it.
            let insert_at = if existing < pos { pos - 1 } else { pos };
            pending.insert(insert_at, target.to_string());
            *cursor = insert_at + 1;
        }
        // Already at the right position — just advance cursor past it.
        else {
            *cursor = pos + 1;
        }
    } else {
        pending.insert(pos, target.to_string());
        *cursor = pos + 1;
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
