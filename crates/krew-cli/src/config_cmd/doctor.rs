use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;

use krew_config::{
    AgentConfig, CONFIG_FILENAME, McpServerConfig, ProviderConfig, user_config_path,
};

pub async fn run() -> anyhow::Result<()> {
    let user_path = user_config_path();
    let project_path = PathBuf::from(CONFIG_FILENAME);

    let mut pass_count = 0u32;
    let mut fail_count = 0u32;
    let mut warn_count = 0u32;

    // ── Config file status ──────────────────────────────────────────
    println!("=== Configuration Doctor ===\n");

    // Parse configs ourselves (don't use UserConfig::load() silent fallback).
    let mut user_providers: HashMap<String, ProviderConfig> = HashMap::new();
    let mut user_mcp_servers: Vec<McpServerConfig> = Vec::new();
    let mut user_ok = false;
    let mut user_parse_failed = false;
    let mut project_ok = false;
    let mut project_agents: Vec<AgentConfig> = Vec::new();
    let mut project_providers: HashMap<String, ProviderConfig> = HashMap::new();
    let mut project_mcp_servers: Vec<McpServerConfig> = Vec::new();

    // User config.
    if let Some(ref path) = user_path {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match toml::from_str::<UserConfigRaw>(&content) {
                    Ok(cfg) => {
                        println!("✅ User config: {}", path.display());
                        user_providers = cfg.providers;
                        user_mcp_servers = cfg.mcp_servers;
                        user_ok = true;
                        pass_count += 1;
                    }
                    Err(e) => {
                        println!("❌ User config: {} (parse error: {})", path.display(), e);
                        user_parse_failed = true;
                        fail_count += 1;
                    }
                },
                Err(e) => {
                    println!("❌ User config: {} (read error: {})", path.display(), e);
                    fail_count += 1;
                }
            }
        } else {
            println!("❌ User config: {} (not found)", path.display());
            fail_count += 1;
        }
    } else {
        println!("❌ User config: cannot determine path");
        fail_count += 1;
    }

    // Project config.
    if project_path.exists() {
        match std::fs::read_to_string(&project_path) {
            Ok(content) => match toml::from_str::<ProjectConfigRaw>(&content) {
                Ok(cfg) => {
                    println!("✅ Project config: {}", project_path.display());
                    project_agents = cfg.agents;
                    project_providers = cfg.providers;
                    project_mcp_servers = cfg.mcp_servers;
                    project_ok = true;
                    pass_count += 1;
                }
                Err(e) => {
                    println!(
                        "❌ Project config: {} (parse error: {})",
                        project_path.display(),
                        e
                    );
                    fail_count += 1;
                }
            },
            Err(e) => {
                println!(
                    "❌ Project config: {} (read error: {})",
                    project_path.display(),
                    e
                );
                fail_count += 1;
            }
        }
    } else {
        println!("❌ Project config: {} (not found)", project_path.display());
        fail_count += 1;
    }

    // Check if both missing.
    if !user_ok
        && !project_ok
        && user_path.as_ref().is_none_or(|p| !p.exists())
        && !project_path.exists()
    {
        println!("\nNo configuration files found. Run `krew config init` to get started.");
        return Ok(());
    }

    // Merge providers: user as base, project overrides.
    let mut merged_providers = user_providers;
    for (key, val) in project_providers {
        merged_providers.insert(key, val);
    }

    // Merge MCP servers: user first, project appended, same-name uses project's.
    let project_mcp_names: std::collections::HashSet<&str> = project_mcp_servers
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    let mut mcp_servers: Vec<McpServerConfig> = user_mcp_servers
        .into_iter()
        .filter(|s| !project_mcp_names.contains(s.name.as_str()))
        .collect();
    mcp_servers.extend(project_mcp_servers);

    // ── Provider diagnostics ────────────────────────────────────────
    if !merged_providers.is_empty() {
        println!("\n--- Providers ---");
        for (name, cfg) in &merged_providers {
            let status = check_provider_key(cfg);
            match status {
                KeyStatus::Set(detail) => {
                    println!("✅ {} — {}", name, detail);
                    pass_count += 1;
                }
                KeyStatus::NotSet(detail) => {
                    println!("❌ {} — {}", name, detail);
                    fail_count += 1;
                }
            }
        }
    }

    // ── Agent diagnostics ───────────────────────────────────────────
    let mut agent_available = 0u32;
    let mut agent_total = 0u32;
    let mut provider_issues = 0u32;
    if project_ok && !project_agents.is_empty() {
        println!("\n--- Agents ---");
        agent_total = project_agents.len() as u32;
        for agent in &project_agents {
            if let Some(pcfg) = merged_providers.get(&agent.provider) {
                let key_ok = matches!(check_provider_key(pcfg), KeyStatus::Set(_));
                if key_ok {
                    println!(
                        "✅ {} — provider: {} ✓, model: {}",
                        agent.name, agent.provider, agent.model
                    );
                    pass_count += 1;
                    agent_available += 1;
                } else {
                    println!(
                        "⚠️  {} — provider: {} (API key not set)",
                        agent.name, agent.provider
                    );
                    warn_count += 1;
                    provider_issues += 1;
                }
            } else if user_parse_failed {
                // User config TOML is corrupted — provider might exist there.
                println!(
                    "⚠️  {} — provider: {} (cannot verify, user config unavailable)",
                    agent.name, agent.provider
                );
                warn_count += 1;
            } else {
                println!(
                    "❌ {} — provider: {} (not found)",
                    agent.name, agent.provider
                );
                fail_count += 1;
                provider_issues += 1;
            }
        }
    }

    // ── MCP server diagnostics ──────────────────────────────────────
    if !mcp_servers.is_empty() {
        println!("\n--- MCP Servers ---");
        for server in &mcp_servers {
            if let Some(ref url) = server.url {
                println!("✅ {} — url: {}", server.name, url);
                pass_count += 1;
            } else if let Some(ref cmd) = server.command {
                let found = which_command(cmd);
                if found {
                    println!("✅ {} — command: {}", server.name, cmd);
                    pass_count += 1;
                } else {
                    println!("⚠️  {} — command: {} (not found in PATH)", server.name, cmd);
                    warn_count += 1;
                }
            }
        }
    }

    // ── Summary ─────────────────────────────────────────────────────
    println!();
    if fail_count == 0 && warn_count == 0 {
        println!("All checks passed. Configuration is ready.");
    } else if agent_total > 0 {
        let mut parts = Vec::new();
        parts.push(format!(
            "{}/{} agents available",
            agent_available, agent_total
        ));
        if provider_issues > 0 {
            parts.push(format!(
                "{} provider{} needs configuration",
                provider_issues,
                if provider_issues > 1 { "s" } else { "" }
            ));
        }
        println!("Result: {}", parts.join(", "));
    } else {
        let total = pass_count + fail_count + warn_count;
        println!(
            "Result: {}/{} checks passed, {} failed, {} warnings",
            pass_count, total, fail_count, warn_count
        );
    }

    Ok(())
}

// ── Internal types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct UserConfigRaw {
    #[serde(default)]
    providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    mcp_servers: Vec<McpServerConfig>,
}

#[derive(Deserialize)]
struct ProjectConfigRaw {
    #[serde(default)]
    agents: Vec<AgentConfig>,
    #[serde(default)]
    providers: HashMap<String, ProviderConfig>,
    #[serde(default)]
    mcp_servers: Vec<McpServerConfig>,
}

enum KeyStatus {
    Set(String),
    NotSet(String),
}

fn check_provider_key(cfg: &ProviderConfig) -> KeyStatus {
    if let Some(ref env) = cfg.api_key_env {
        if std::env::var(env).is_ok() {
            KeyStatus::Set(format!("{} is set", env))
        } else {
            KeyStatus::NotSet(format!("{} not set", env))
        }
    } else if cfg.api_key.is_some() {
        KeyStatus::Set("API key configured (config file)".to_string())
    } else {
        KeyStatus::NotSet("no API key configured".to_string())
    }
}

fn which_command(cmd: &str) -> bool {
    // Check if command exists in PATH.
    #[cfg(unix)]
    {
        std::process::Command::new("which")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
    #[cfg(windows)]
    {
        std::process::Command::new("where")
            .arg(cmd)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    }
    #[cfg(not(any(unix, windows)))]
    {
        false
    }
}
