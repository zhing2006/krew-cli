//! Format-preserving TOML configuration writer using `toml_edit`.
//!
//! Provides functions to add/remove providers and agents in krew config files
//! while preserving existing comments and formatting.

use std::path::Path;

use toml_edit::{Array, DocumentMut, Item, Table};

use crate::{AgentConfig, ConfigError, ProviderConfig, ProviderType};

// ── Document helpers ────────────────────────────────────────────────

/// Load a TOML document from a file. Returns an empty document if the file
/// does not exist.
pub fn load_document(path: &Path) -> Result<DocumentMut, ConfigError> {
    if !path.exists() {
        return Ok(DocumentMut::new());
    }
    let content = std::fs::read_to_string(path)?;
    content
        .parse::<DocumentMut>()
        .map_err(|e| ConfigError::Parse(e.to_string()))
}

/// Save a TOML document to a file, creating parent directories if needed.
pub fn save_document(path: &Path, doc: &DocumentMut) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, doc.to_string())?;
    Ok(())
}

// ── Provider operations ─────────────────────────────────────────────

/// Data for adding a provider.
pub struct ProviderWriteData {
    pub name: String,
    pub provider_type: ProviderType,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
    pub vertex_project: Option<String>,
    pub vertex_location: Option<String>,
    pub extra_headers: Option<std::collections::HashMap<String, String>>,
}

/// Add a provider to the given config file.
pub fn add_provider(path: &Path, data: &ProviderWriteData) -> Result<(), ConfigError> {
    let mut doc = load_document(path)?;

    // Ensure [providers] table exists.
    if !doc.contains_table("providers") {
        doc["providers"] = Item::Table(Table::new());
    }

    let providers = doc["providers"]
        .as_table_mut()
        .ok_or_else(|| ConfigError::Validation("'providers' is not a table".to_string()))?;

    if providers.contains_key(&data.name) {
        return Err(ConfigError::Validation(format!(
            "provider \"{}\" already exists",
            data.name
        )));
    }

    let mut table = Table::new();
    table.set_implicit(true);

    table["type"] = toml_edit::value(data.provider_type.as_str());

    if let Some(ref key) = data.api_key {
        table["api_key"] = toml_edit::value(key.as_str());
    }
    if let Some(ref env) = data.api_key_env {
        table["api_key_env"] = toml_edit::value(env.as_str());
    }
    if let Some(ref url) = data.base_url {
        table["base_url"] = toml_edit::value(url.as_str());
    }
    if let Some(ref proj) = data.vertex_project {
        table["vertex_project"] = toml_edit::value(proj.as_str());
    }
    if let Some(ref loc) = data.vertex_location {
        table["vertex_location"] = toml_edit::value(loc.as_str());
    }
    if let Some(ref headers) = data.extra_headers {
        let mut inline = toml_edit::InlineTable::new();
        for (k, v) in headers {
            inline.insert(k.as_str(), v.as_str().into());
        }
        table["extra_headers"] = toml_edit::value(inline);
    }

    providers[&data.name] = Item::Table(table);
    save_document(path, &doc)
}

/// Remove a provider from the given config file.
pub fn remove_provider(path: &Path, name: &str) -> Result<(), ConfigError> {
    let mut doc = load_document(path)?;

    let providers = doc
        .get_mut("providers")
        .and_then(|p| p.as_table_mut())
        .ok_or_else(|| ConfigError::Validation(format!("provider \"{name}\" not found")))?;

    if !providers.contains_key(name) {
        return Err(ConfigError::Validation(format!(
            "provider \"{name}\" not found"
        )));
    }

    providers.remove(name);
    save_document(path, &doc)
}

// ── Agent operations ────────────────────────────────────────────────

/// Data for adding an agent.
pub struct AgentWriteData {
    pub name: String,
    pub display_name: String,
    pub provider: String,
    pub model: String,
    pub color: String,
    pub enable_thinking: bool,
    pub thinking_effort: Option<String>,
    pub enable_web_search: bool,
    pub tools: bool,
    pub api_type: Option<String>,
    pub system_prompt: Option<String>,
}

/// Add an agent to the given config file and append to reply_order.
pub fn add_agent(path: &Path, data: &AgentWriteData) -> Result<(), ConfigError> {
    let mut doc = load_document(path)?;

    // Check for duplicate agent name.
    if let Some(agents) = doc.get("agents").and_then(|a| a.as_array_of_tables()) {
        for agent in agents.iter() {
            if agent.get("name").and_then(|n| n.as_str()) == Some(&data.name) {
                return Err(ConfigError::Validation(format!(
                    "agent \"{}\" already exists",
                    data.name
                )));
            }
        }
    }

    append_agent_to_doc(&mut doc, data);
    append_to_reply_order(&mut doc, &data.name);
    save_document(path, &doc)
}

/// Remove an agent from the given config file and update reply_order.
pub fn remove_agent(path: &Path, name: &str) -> Result<(), ConfigError> {
    let mut doc = load_document(path)?;

    let agents = doc
        .get_mut("agents")
        .and_then(|a| a.as_array_of_tables_mut())
        .ok_or_else(|| ConfigError::Validation(format!("agent \"{name}\" not found")))?;

    let idx = agents
        .iter()
        .position(|a| a.get("name").and_then(|n| n.as_str()) == Some(name))
        .ok_or_else(|| ConfigError::Validation(format!("agent \"{name}\" not found")))?;

    agents.remove(idx);

    // Update reply_order.
    remove_from_reply_order(&mut doc, name);

    save_document(path, &doc)
}

/// Batch-add multiple agents (for init preset). Refuses if agents already exist.
pub fn batch_add_agents(path: &Path, agents: &[AgentWriteData]) -> Result<(), ConfigError> {
    let mut doc = load_document(path)?;

    // Check if agents already exist in the document.
    if doc
        .get("agents")
        .and_then(|a| a.as_array_of_tables())
        .is_some_and(|arr| !arr.is_empty())
    {
        return Err(ConfigError::Validation(
            "agents already exist in config file; use `krew config add agent` instead".to_string(),
        ));
    }

    // Build reply_order from agent names.
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();

    for data in agents {
        append_agent_to_doc(&mut doc, data);
    }

    // Set reply_order.
    ensure_settings_table(&mut doc);
    let settings = doc["settings"].as_table_mut().unwrap();
    let mut arr = Array::new();
    for name in &names {
        arr.push(*name);
    }
    settings["reply_order"] = toml_edit::value(arr);

    save_document(path, &doc)
}

// ── List operations ─────────────────────────────────────────────────

/// List providers from a config file.
pub fn list_providers(path: &Path) -> Result<Vec<(String, ProviderConfig)>, ConfigError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)?;
    let raw: RawProviders = toml::from_str(&content)?;
    Ok(raw.providers.into_iter().collect())
}

/// List agents and reply_order from a config file.
pub fn list_agents(path: &Path) -> Result<(Vec<AgentConfig>, Vec<String>), ConfigError> {
    if !path.exists() {
        return Ok((Vec::new(), Vec::new()));
    }
    let content = std::fs::read_to_string(path)?;
    let raw: RawAgents = toml::from_str(&content)?;
    Ok((
        raw.agents,
        raw.settings.map(|s| s.reply_order).unwrap_or_default(),
    ))
}

// ── Internal helpers ────────────────────────────────────────────────

fn append_agent_to_doc(doc: &mut DocumentMut, data: &AgentWriteData) {
    let mut table = Table::new();
    table["name"] = toml_edit::value(&data.name);
    table["display_name"] = toml_edit::value(&data.display_name);
    table["provider"] = toml_edit::value(&data.provider);
    table["model"] = toml_edit::value(&data.model);
    table["color"] = toml_edit::value(&data.color);
    table["enable_thinking"] = toml_edit::value(data.enable_thinking);
    table["enable_web_search"] = toml_edit::value(data.enable_web_search);

    if let Some(ref thinking_effort) = data.thinking_effort {
        table["thinking_effort"] = toml_edit::value(thinking_effort.as_str());
    }

    if let Some(ref api_type) = data.api_type {
        table["api_type"] = toml_edit::value(api_type.as_str());
    }
    if let Some(ref prompt) = data.system_prompt {
        table["system_prompt"] = toml_edit::value(prompt.as_str());
    }

    // Append to [[agents]] array-of-tables.
    if !doc.contains_array_of_tables("agents") {
        // Need to create the array of tables entry.
        doc.insert("agents", Item::ArrayOfTables(Default::default()));
    }
    let arr = doc["agents"].as_array_of_tables_mut().unwrap();
    arr.push(table);
}

fn ensure_settings_table(doc: &mut DocumentMut) {
    if !doc.contains_table("settings") {
        doc["settings"] = Item::Table(Table::new());
    }
}

fn append_to_reply_order(doc: &mut DocumentMut, name: &str) {
    ensure_settings_table(doc);
    let settings = doc["settings"].as_table_mut().unwrap();

    if let Some(arr) = settings
        .get_mut("reply_order")
        .and_then(|v| v.as_value_mut())
        .and_then(|v| v.as_array_mut())
    {
        arr.push(name);
    } else {
        let mut arr = Array::new();
        arr.push(name);
        settings["reply_order"] = toml_edit::value(arr);
    }
}

fn remove_from_reply_order(doc: &mut DocumentMut, name: &str) {
    if let Some(settings) = doc.get_mut("settings").and_then(|s| s.as_table_mut())
        && let Some(arr) = settings
            .get_mut("reply_order")
            .and_then(|v| v.as_value_mut())
            .and_then(|v| v.as_array_mut())
    {
        arr.retain(|v| v.as_str() != Some(name));
    }
}

// Deserialization helpers for list operations.
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct RawProviders {
    #[serde(default)]
    providers: HashMap<String, ProviderConfig>,
}

#[derive(Deserialize)]
struct RawAgents {
    #[serde(default)]
    agents: Vec<AgentConfig>,
    settings: Option<RawAgentsSettings>,
}

#[derive(Deserialize)]
struct RawAgentsSettings {
    #[serde(default)]
    reply_order: Vec<String>,
}
