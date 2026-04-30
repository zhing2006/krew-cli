use std::path::PathBuf;

use crate::AddTarget;
use crate::config_cmd::init;
use krew_config::{CONFIG_FILENAME, user_config_path};

pub async fn run(target: AddTarget) -> anyhow::Result<()> {
    match target {
        AddTarget::Provider => add_provider().await,
        AddTarget::Agent => add_agent().await,
    }
}

async fn add_provider() -> anyhow::Result<()> {
    let user_path =
        user_config_path().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

    // Get existing provider names for duplicate checking.
    let existing = krew_config::writer::list_providers(&user_path)?;
    let existing_names: Vec<String> = existing.iter().map(|(n, _)| n.clone()).collect();

    let data = init::collect_provider_data(&existing_names)?;
    krew_config::writer::add_provider(&user_path, &data)?;

    println!(
        "Added provider \"{}\" ({})",
        data.name,
        data.provider_type.label()
    );

    Ok(())
}

async fn add_agent() -> anyhow::Result<()> {
    let project_path = PathBuf::from(CONFIG_FILENAME);

    // Get merged providers.
    let providers = init::get_merged_providers()?;
    if providers.is_empty() {
        println!("No providers configured. Run `krew config add provider` first.");
        return Ok(());
    }

    // Get existing agent names and colors for duplicate/used checking.
    let (existing_agents, _) = krew_config::writer::list_agents(&project_path)?;
    let existing_names: Vec<String> = existing_agents.iter().map(|a| a.name.clone()).collect();
    let used_colors: Vec<String> = existing_agents.iter().map(|a| a.color.clone()).collect();

    let data = init::collect_agent_data(&providers, &existing_names, &used_colors).await?;
    krew_config::writer::add_agent(&project_path, &data)?;

    println!("Added agent \"{}\"", data.name);

    Ok(())
}
