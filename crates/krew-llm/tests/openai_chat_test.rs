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
}

#[test]
fn convert_messages_other_agent_as_user() {
    let messages = vec![ChatMessage {
        role: ChatRole::Assistant,
        content: "I suggest using VecDeque...".into(),
        name: Some("opus".into()),
    }];

    let result = convert_messages(&messages, "gpt", &OtherAgentRole::User);
    assert_eq!(result[0]["role"], "user"); // other agent → user role
}

#[test]
fn convert_messages_other_agent_as_assistant() {
    let messages = vec![ChatMessage {
        role: ChatRole::Assistant,
        content: "I suggest using VecDeque...".into(),
        name: Some("opus".into()),
    }];

    let result = convert_messages(&messages, "gpt", &OtherAgentRole::Assistant);
    assert_eq!(result[0]["role"], "assistant"); // other agent → assistant role
}
