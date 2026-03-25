use std::path::PathBuf;

use dialoguer::{Confirm, Select};

use crate::DelTarget;
use krew_config::{CONFIG_FILENAME, user_config_path};

pub async fn run(target: DelTarget) -> anyhow::Result<()> {
    match target {
        DelTarget::Provider => del_provider().await,
        DelTarget::Agent => del_agent().await,
    }
}

async fn del_provider() -> anyhow::Result<()> {
    let user_path =
        user_config_path().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

    let providers = krew_config::writer::list_providers(&user_path)?;
    if providers.is_empty() {
        println!("No providers to delete.");
        return Ok(());
    }

    let labels: Vec<String> = providers
        .iter()
        .map(|(n, c)| {
            format!(
                "{} ({})",
                n,
                match c.provider_type {
                    krew_config::ProviderType::OpenAI => "OpenAI",
                    krew_config::ProviderType::Anthropic => "Anthropic",
                    krew_config::ProviderType::Google => "Google",
                }
            )
        })
        .collect();

    let idx = Select::new()
        .with_prompt("Select provider to delete")
        .items(&labels)
        .interact()?;

    let (name, _) = &providers[idx];

    // Check if any project agents reference this provider.
    let project_path = PathBuf::from(CONFIG_FILENAME);
    let (agents, _) = krew_config::writer::list_agents(&project_path)?;
    let referencing: Vec<&str> = agents
        .iter()
        .filter(|a| &a.provider == name)
        .map(|a| a.name.as_str())
        .collect();

    if !referencing.is_empty() {
        println!(
            "Warning: The following agents use this provider: {}",
            referencing.join(", ")
        );
    }

    let confirm = Confirm::new()
        .with_prompt(format!("Delete provider \"{}\"?", name))
        .default(false)
        .interact()?;

    if !confirm {
        println!("Cancelled.");
        return Ok(());
    }

    krew_config::writer::remove_provider(&user_path, name)?;
    println!("Deleted provider \"{}\"", name);

    Ok(())
}

async fn del_agent() -> anyhow::Result<()> {
    let project_path = PathBuf::from(CONFIG_FILENAME);

    let (agents, _) = krew_config::writer::list_agents(&project_path)?;
    if agents.is_empty() {
        println!("No agents to delete.");
        return Ok(());
    }

    let labels: Vec<String> = agents
        .iter()
        .map(|a| format!("{} ({}/{})", a.name, a.provider, a.model))
        .collect();

    let idx = Select::new()
        .with_prompt("Select agent to delete")
        .items(&labels)
        .interact()?;

    let agent_name = &agents[idx].name;

    if agents.len() == 1 {
        println!("Warning: This is the last agent. Deleting it will prevent krew from starting.");
    }

    let confirm = Confirm::new()
        .with_prompt(format!("Delete agent \"{}\"?", agent_name))
        .default(false)
        .interact()?;

    if !confirm {
        println!("Cancelled.");
        return Ok(());
    }

    krew_config::writer::remove_agent(&project_path, agent_name)?;
    println!("Deleted agent \"{}\"", agent_name);

    Ok(())
}
