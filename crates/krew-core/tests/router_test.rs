use krew_core::router::{Addressee, parse_input};

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
fn parse_all_mixed_with_specific() {
    let (addr, msg) = parse_input("@gpt @all hello", &agents()).unwrap();
    assert_eq!(addr, Addressee::All);
    assert_eq!(msg, "@gpt @all hello");
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
