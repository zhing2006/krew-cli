use std::path::PathBuf;

use crate::ListTarget;
use krew_config::{CONFIG_FILENAME, user_config_path};

pub async fn run(target: ListTarget) -> anyhow::Result<()> {
    match target {
        ListTarget::Providers => list_providers().await,
        ListTarget::Agents => list_agents().await,
    }
}

async fn list_providers() -> anyhow::Result<()> {
    let user_path =
        user_config_path().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;

    let providers = krew_config::writer::list_providers(&user_path)?;
    if providers.is_empty() {
        println!("No providers configured. Run `krew config add provider` to add one.");
        return Ok(());
    }

    println!(
        "{:<16} {:<12} {:<28} Base URL",
        "Name", "Type", "Key Method"
    );
    println!("{}", "-".repeat(76));

    for (name, cfg) in &providers {
        let type_label = cfg.provider_type.label();

        let key_method = if let Some(ref env) = cfg.api_key_env {
            let set = std::env::var(env).is_ok();
            format!("env: {} {}", env, if set { "✅" } else { "❌" })
        } else if cfg.api_key.is_some() {
            "config file".to_string()
        } else {
            "none".to_string()
        };

        let base = cfg.base_url.as_deref().unwrap_or("-");

        println!(
            "{:<16} {:<12} {:<28} {}",
            name, type_label, key_method, base
        );
    }

    Ok(())
}

async fn list_agents() -> anyhow::Result<()> {
    let project_path = PathBuf::from(CONFIG_FILENAME);

    let (agents, reply_order) = krew_config::writer::list_agents(&project_path)?;
    if agents.is_empty() {
        println!(
            "No agents configured. Run `krew config init` or `krew config add agent` to add one."
        );
        return Ok(());
    }

    println!(
        "{:<12} {:<12} {:<12} {:<24} {:<10} {:<9} Web",
        "Name", "Display", "Provider", "Model", "Color", "Thinking"
    );
    println!("{}", "-".repeat(90));

    for a in &agents {
        println!(
            "{:<12} {:<12} {:<12} {:<24} {:<10} {:<9} {}",
            a.name,
            a.display_name,
            a.provider,
            a.model,
            a.color,
            if a.enable_thinking { "yes" } else { "no" },
            if a.enable_web_search { "yes" } else { "no" },
        );
    }

    if !reply_order.is_empty() {
        println!("\nReply order: {}", reply_order.join(" -> "));
    }

    Ok(())
}
