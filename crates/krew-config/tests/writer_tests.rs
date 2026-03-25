use krew_config::ProviderType;
use krew_config::writer::*;

fn temp_file(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    if !content.is_empty() {
        std::fs::write(&path, content).unwrap();
    }
    (dir, path)
}

// ── Provider tests ──────────────────────────────────────────────────

#[test]
fn add_provider_to_empty_file() {
    let (dir, path) = temp_file("");
    add_provider(
        &path,
        &ProviderWriteData {
            name: "anthropic".into(),
            provider_type: ProviderType::Anthropic,
            api_key: None,
            api_key_env: Some("ANTHROPIC_API_KEY".into()),
            base_url: None,
            vertex_project: None,
            vertex_location: None,
        },
    )
    .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("[providers.anthropic]"));
    assert!(content.contains("type = \"anthropic\""));
    assert!(content.contains("api_key_env = \"ANTHROPIC_API_KEY\""));
    assert!(!content.contains("api_key ="));
    drop(dir);
}

#[test]
fn add_provider_to_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("subdir").join("settings.toml");
    add_provider(
        &path,
        &ProviderWriteData {
            name: "openai".into(),
            provider_type: ProviderType::OpenAI,
            api_key: Some("sk-test123".into()),
            api_key_env: None,
            base_url: None,
            vertex_project: None,
            vertex_location: None,
        },
    )
    .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("[providers.openai]"));
    assert!(content.contains("api_key = \"sk-test123\""));
    drop(dir);
}

#[test]
fn add_provider_preserves_comments() {
    let (dir, path) = temp_file(
        "# This is my config\n\
         [providers.existing]\n\
         type = \"anthropic\"\n\
         api_key_env = \"KEY\"\n",
    );

    add_provider(
        &path,
        &ProviderWriteData {
            name: "openai".into(),
            provider_type: ProviderType::OpenAI,
            api_key: Some("sk-xxx".into()),
            api_key_env: None,
            base_url: None,
            vertex_project: None,
            vertex_location: None,
        },
    )
    .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# This is my config"));
    assert!(content.contains("[providers.existing]"));
    assert!(content.contains("[providers.openai]"));
    drop(dir);
}

#[test]
fn add_provider_duplicate_name_errors() {
    let (dir, path) = temp_file(
        "[providers.openai]\n\
         type = \"openai\"\n\
         api_key = \"sk-x\"\n",
    );

    let result = add_provider(
        &path,
        &ProviderWriteData {
            name: "openai".into(),
            provider_type: ProviderType::OpenAI,
            api_key: Some("sk-y".into()),
            api_key_env: None,
            base_url: None,
            vertex_project: None,
            vertex_location: None,
        },
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
    drop(dir);
}

#[test]
fn add_provider_with_vertex_fields() {
    let (dir, path) = temp_file("");
    add_provider(
        &path,
        &ProviderWriteData {
            name: "gcp".into(),
            provider_type: ProviderType::Google,
            api_key: Some("token".into()),
            api_key_env: None,
            base_url: None,
            vertex_project: Some("my-project".into()),
            vertex_location: Some("us-central1".into()),
        },
    )
    .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("vertex_project = \"my-project\""));
    assert!(content.contains("vertex_location = \"us-central1\""));
    drop(dir);
}

#[test]
fn add_provider_with_base_url() {
    let (dir, path) = temp_file("");
    add_provider(
        &path,
        &ProviderWriteData {
            name: "deepseek".into(),
            provider_type: ProviderType::OpenAI,
            api_key: Some("sk-ds".into()),
            api_key_env: None,
            base_url: Some("https://api.deepseek.com".into()),
            vertex_project: None,
            vertex_location: None,
        },
    )
    .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("base_url = \"https://api.deepseek.com\""));
    drop(dir);
}

#[test]
fn remove_provider_success() {
    let (dir, path) = temp_file(
        "[providers.openai]\n\
         type = \"openai\"\n\
         api_key = \"sk-x\"\n\n\
         [providers.anthropic]\n\
         type = \"anthropic\"\n\
         api_key = \"sk-y\"\n",
    );

    remove_provider(&path, "openai").unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(!content.contains("[providers.openai]"));
    assert!(content.contains("[providers.anthropic]"));
    drop(dir);
}

#[test]
fn remove_provider_not_found() {
    let (dir, path) = temp_file(
        "[providers.openai]\n\
         type = \"openai\"\n",
    );
    let result = remove_provider(&path, "nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
    drop(dir);
}

// ── Agent tests ─────────────────────────────────────────────────────

#[test]
fn add_agent_to_empty_file() {
    let (dir, path) = temp_file("");
    add_agent(
        &path,
        &AgentWriteData {
            name: "claude".into(),
            display_name: "Claude".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-6".into(),
            color: "blue".into(),
            enable_thinking: true,
            enable_web_search: false,
            tools: true,
            api_type: None,
            system_prompt: None,
        },
    )
    .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("[[agents]]"));
    assert!(content.contains("name = \"claude\""));
    assert!(content.contains("enable_thinking = true"));
    assert!(content.contains("reply_order"));
    drop(dir);
}

#[test]
fn add_agent_appends_to_existing() {
    let (dir, path) = temp_file(
        "[settings]\n\
         reply_order = [\"claude\"]\n\n\
         [[agents]]\n\
         name = \"claude\"\n\
         display_name = \"Claude\"\n\
         provider = \"anthropic\"\n\
         model = \"claude-sonnet-4-6\"\n\
         color = \"blue\"\n\
         enable_thinking = true\n\
         enable_web_search = false\n",
    );

    add_agent(
        &path,
        &AgentWriteData {
            name: "gpt".into(),
            display_name: "GPT".into(),
            provider: "openai".into(),
            model: "gpt-5.4".into(),
            color: "green".into(),
            enable_thinking: false,
            enable_web_search: true,
            tools: true,
            api_type: Some("chat".into()),
            system_prompt: None,
        },
    )
    .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("name = \"gpt\""));
    assert!(content.contains("api_type = \"chat\""));
    // reply_order should now include both.
    assert!(content.contains("claude"));
    assert!(content.contains("gpt"));
    drop(dir);
}

#[test]
fn add_agent_duplicate_name_errors() {
    let (dir, path) = temp_file(
        "[[agents]]\n\
         name = \"claude\"\n\
         display_name = \"Claude\"\n\
         provider = \"anthropic\"\n\
         model = \"m\"\n\
         color = \"blue\"\n\
         enable_thinking = true\n\
         enable_web_search = false\n",
    );

    let result = add_agent(
        &path,
        &AgentWriteData {
            name: "claude".into(),
            display_name: "Claude".into(),
            provider: "anthropic".into(),
            model: "m".into(),
            color: "blue".into(),
            enable_thinking: true,
            enable_web_search: false,
            tools: true,
            api_type: None,
            system_prompt: None,
        },
    );

    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
    drop(dir);
}

#[test]
fn remove_agent_success() {
    let (dir, path) = temp_file(
        "[settings]\n\
         reply_order = [\"claude\", \"gpt\"]\n\n\
         [[agents]]\n\
         name = \"claude\"\n\
         display_name = \"Claude\"\n\
         provider = \"anthropic\"\n\
         model = \"m\"\n\
         color = \"blue\"\n\
         enable_thinking = true\n\
         enable_web_search = false\n\n\
         [[agents]]\n\
         name = \"gpt\"\n\
         display_name = \"GPT\"\n\
         provider = \"openai\"\n\
         model = \"m\"\n\
         color = \"green\"\n\
         enable_thinking = false\n\
         enable_web_search = false\n",
    );

    remove_agent(&path, "claude").unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(!content.contains("name = \"claude\""));
    assert!(content.contains("name = \"gpt\""));
    // reply_order should only have gpt.
    assert!(!content.contains("\"claude\""));
    drop(dir);
}

#[test]
fn remove_agent_not_found() {
    let (dir, path) = temp_file(
        "[[agents]]\n\
         name = \"claude\"\n\
         display_name = \"Claude\"\n\
         provider = \"anthropic\"\n\
         model = \"m\"\n\
         color = \"blue\"\n\
         enable_thinking = true\n\
         enable_web_search = false\n",
    );

    let result = remove_agent(&path, "nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
    drop(dir);
}

// ── Batch add tests ─────────────────────────────────────────────────

#[test]
fn batch_add_agents_to_new_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");

    let agents = vec![
        AgentWriteData {
            name: "claude".into(),
            display_name: "Claude".into(),
            provider: "anthropic".into(),
            model: "claude-sonnet-4-6".into(),
            color: "blue".into(),
            enable_thinking: true,
            enable_web_search: false,
            tools: true,
            api_type: None,
            system_prompt: None,
        },
        AgentWriteData {
            name: "gpt".into(),
            display_name: "GPT".into(),
            provider: "openai".into(),
            model: "gpt-5.4".into(),
            color: "green".into(),
            enable_thinking: true,
            enable_web_search: false,
            tools: true,
            api_type: None,
            system_prompt: None,
        },
        AgentWriteData {
            name: "gemini".into(),
            display_name: "Gemini".into(),
            provider: "google".into(),
            model: "gemini-3.1-pro-preview".into(),
            color: "cyan".into(),
            enable_thinking: true,
            enable_web_search: false,
            tools: true,
            api_type: None,
            system_prompt: None,
        },
    ];

    batch_add_agents(&path, &agents).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("name = \"claude\""));
    assert!(content.contains("name = \"gpt\""));
    assert!(content.contains("name = \"gemini\""));
    assert!(content.contains("reply_order"));
    drop(dir);
}

#[test]
fn batch_add_agents_refuses_if_agents_exist() {
    let (dir, path) = temp_file(
        "[[agents]]\n\
         name = \"existing\"\n\
         display_name = \"Existing\"\n\
         provider = \"p\"\n\
         model = \"m\"\n\
         color = \"blue\"\n\
         enable_thinking = true\n\
         enable_web_search = false\n",
    );

    let agents = vec![AgentWriteData {
        name: "new".into(),
        display_name: "New".into(),
        provider: "p".into(),
        model: "m".into(),
        color: "green".into(),
        enable_thinking: true,
        enable_web_search: false,
        tools: true,
        api_type: None,
        system_prompt: None,
    }];

    let result = batch_add_agents(&path, &agents);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exist"));
    drop(dir);
}

#[test]
fn batch_add_agents_ok_if_file_exists_but_no_agents() {
    let (dir, path) = temp_file(
        "[settings]\n\
         approval_mode = \"suggest\"\n",
    );

    let agents = vec![AgentWriteData {
        name: "claude".into(),
        display_name: "Claude".into(),
        provider: "anthropic".into(),
        model: "m".into(),
        color: "blue".into(),
        enable_thinking: true,
        enable_web_search: false,
        tools: true,
        api_type: None,
        system_prompt: None,
    }];

    batch_add_agents(&path, &agents).unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("approval_mode = \"suggest\""));
    assert!(content.contains("name = \"claude\""));
    drop(dir);
}

// ── List tests ──────────────────────────────────────────────────────

#[test]
fn list_providers_from_file() {
    let (dir, path) = temp_file(
        "[providers.anthropic]\n\
         type = \"anthropic\"\n\
         api_key_env = \"KEY\"\n\n\
         [providers.openai]\n\
         type = \"openai\"\n\
         api_key = \"sk-x\"\n",
    );

    let providers = list_providers(&path).unwrap();
    assert_eq!(providers.len(), 2);
    drop(dir);
}

#[test]
fn list_providers_file_not_exists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonexistent.toml");
    let providers = list_providers(&path).unwrap();
    assert!(providers.is_empty());
    drop(dir);
}

#[test]
fn list_agents_from_file() {
    let (dir, path) = temp_file(
        "[settings]\n\
         reply_order = [\"claude\", \"gpt\"]\n\n\
         [[agents]]\n\
         name = \"claude\"\n\
         display_name = \"Claude\"\n\
         provider = \"anthropic\"\n\
         model = \"m\"\n\
         color = \"blue\"\n\
         enable_thinking = true\n\
         enable_web_search = false\n\n\
         [[agents]]\n\
         name = \"gpt\"\n\
         display_name = \"GPT\"\n\
         provider = \"openai\"\n\
         model = \"m\"\n\
         color = \"green\"\n\
         enable_thinking = false\n\
         enable_web_search = false\n",
    );

    let (agents, reply_order) = list_agents(&path).unwrap();
    assert_eq!(agents.len(), 2);
    assert_eq!(reply_order, vec!["claude", "gpt"]);
    drop(dir);
}

#[test]
fn list_agents_file_not_exists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nonexistent.toml");
    let (agents, reply_order) = list_agents(&path).unwrap();
    assert!(agents.is_empty());
    assert!(reply_order.is_empty());
    drop(dir);
}

// ── Format preservation ─────────────────────────────────────────────

#[test]
fn format_preservation_after_add_remove() {
    let original = "# Global settings\n\
         [settings]\n\
         approval_mode = \"suggest\"\n\
         reply_order = [\"claude\"]\n\n\
         # Agent definitions\n\
         [[agents]]\n\
         name = \"claude\"\n\
         display_name = \"Claude\"\n\
         provider = \"anthropic\"\n\
         model = \"m\"\n\
         color = \"blue\"\n\
         enable_thinking = true\n\
         enable_web_search = false\n";

    let (dir, path) = temp_file(original);

    // Add an agent.
    add_agent(
        &path,
        &AgentWriteData {
            name: "gpt".into(),
            display_name: "GPT".into(),
            provider: "openai".into(),
            model: "gpt-5.4".into(),
            color: "green".into(),
            enable_thinking: false,
            enable_web_search: false,
            tools: true,
            api_type: None,
            system_prompt: None,
        },
    )
    .unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    // Comments should be preserved.
    assert!(content.contains("# Global settings"));
    assert!(content.contains("# Agent definitions"));

    // Remove the added agent.
    remove_agent(&path, "gpt").unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("# Global settings"));
    assert!(content.contains("# Agent definitions"));
    assert!(content.contains("name = \"claude\""));
    assert!(!content.contains("name = \"gpt\""));
    drop(dir);
}

// ── Corrupted file tests ────────────────────────────────────────────

#[test]
fn load_document_with_corrupted_toml() {
    let (dir, path) = temp_file("this is not valid [toml\n");
    let result = load_document(&path);
    assert!(result.is_err());
    drop(dir);
}
