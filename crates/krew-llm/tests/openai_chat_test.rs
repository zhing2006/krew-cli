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

    let result = convert_messages(&messages, "gpt", &OtherAgentRole::User);
    assert_eq!(result[0]["role"], "system");
    assert_eq!(result[1]["role"], "user");
    assert_eq!(result[2]["role"], "assistant"); // own message
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

    let result = convert_messages(&messages, "gpt", &OtherAgentRole::User);
    assert_eq!(result[0]["role"], "user");
    // Content prefixed with [agent_name].
    assert_eq!(result[0]["content"], "[opus] I suggest using VecDeque...");
}

#[test]
fn convert_messages_other_agent_as_assistant() {
    let messages = vec![ChatMessage {
        role: ChatRole::Assistant,
        content: "I suggest using VecDeque...".into(),
        name: Some("opus".into()),
    }];

    let result = convert_messages(&messages, "gpt", &OtherAgentRole::Assistant);
    assert_eq!(result[0]["role"], "assistant");
    // Content still prefixed with [agent_name] for disambiguation.
    assert_eq!(result[0]["content"], "[opus] I suggest using VecDeque...");
}

#[test]
fn convert_messages_user_no_prefix() {
    let messages = vec![ChatMessage {
        role: ChatRole::User,
        content: "Hello".into(),
        name: None,
    }];

    let result = convert_messages(&messages, "gpt", &OtherAgentRole::User);
    assert_eq!(result[0]["role"], "user");
    assert_eq!(result[0]["content"], "Hello");
}
