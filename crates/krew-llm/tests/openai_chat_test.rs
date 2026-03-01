use krew_llm::openai_chat::convert_messages;
use krew_llm::{ChatMessage, ChatRole, OtherAgentRole};

#[test]
fn convert_messages_basic_roles() {
    let messages = vec![
        ChatMessage {
            role: ChatRole::System,
            content: "You are helpful.".into(),
            name: None,
        },
        ChatMessage {
            role: ChatRole::User,
            content: "Hello".into(),
            name: None,
        },
        ChatMessage {
            role: ChatRole::Assistant,
            content: "Hi there!".into(),
            name: Some("gpt".into()),
        },
    ];

    let result = convert_messages(&messages, "gpt", &OtherAgentRole::User, false);
    assert_eq!(result[0]["role"], "system");
    assert_eq!(result[1]["role"], "user");
    assert_eq!(result[2]["role"], "assistant"); // own message
    // Own message should not have name field.
    assert!(result[2].get("name").is_none());
    // Own message content should not be prefixed.
    assert_eq!(result[2]["content"], "Hi there!");
}

#[test]
fn convert_messages_other_agent_content_prefix() {
    let messages = vec![ChatMessage {
        role: ChatRole::Assistant,
        content: "I suggest using VecDeque...".into(),
        name: Some("opus".into()),
    }];

    let result = convert_messages(&messages, "gpt", &OtherAgentRole::User, false);
    assert_eq!(result[0]["role"], "user");
    // No name field, content prefixed instead.
    assert!(result[0].get("name").is_none());
    assert_eq!(result[0]["content"], "[opus] I suggest using VecDeque...");
}

#[test]
fn convert_messages_other_agent_name_field() {
    let messages = vec![ChatMessage {
        role: ChatRole::Assistant,
        content: "I suggest using VecDeque...".into(),
        name: Some("opus".into()),
    }];

    let result = convert_messages(&messages, "gpt", &OtherAgentRole::User, true);
    assert_eq!(result[0]["role"], "user");
    // Name field set, content not prefixed.
    assert_eq!(result[0]["name"], "opus");
    assert_eq!(result[0]["content"], "I suggest using VecDeque...");
}

#[test]
fn convert_messages_user_no_name_field() {
    let messages = vec![ChatMessage {
        role: ChatRole::User,
        content: "Hello".into(),
        name: None,
    }];

    let result = convert_messages(&messages, "gpt", &OtherAgentRole::User, true);
    assert_eq!(result[0]["role"], "user");
    assert!(result[0].get("name").is_none());
}
