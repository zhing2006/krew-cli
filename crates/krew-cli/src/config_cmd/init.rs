use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use dialoguer::{Confirm, FuzzySelect, Input, Password, Select};
use krew_config::writer::{AgentWriteData, ProviderWriteData};
use krew_config::{CONFIG_FILENAME, ProviderConfig, ProviderType, user_config_path};
use krew_llm::{ListModelsConfig, fallback_models, list_models};

/// Available colors for agent assignment.
const AGENT_COLORS: &[&str] = &["blue", "green", "cyan", "magenta", "yellow", "red", "white"];

pub async fn run(user_only: bool, project_only: bool) -> anyhow::Result<()> {
    let user_path = user_config_path().context("Cannot determine home directory")?;
    let project_path = PathBuf::from(CONFIG_FILENAME);

    let user_exists = user_path.exists();
    let project_exists = project_path.exists();

    if user_only {
        if user_exists {
            println!("User config already exists. Use `krew config add/del provider` to modify.");
            return Ok(());
        }
        run_user_init(&user_path).await?;
        return Ok(());
    }

    if project_only {
        if project_exists {
            println!("Project config already exists. Use `krew config add/del agent` to modify.");
            return Ok(());
        }
        run_project_init(&project_path).await?;
        return Ok(());
    }

    // Smart routing based on config existence.
    match (user_exists, project_exists) {
        (false, false) => {
            run_user_init(&user_path).await?;
            // Offer to continue with project init.
            let continue_project = Confirm::new()
                .with_prompt("Initialize agent configuration for current project?")
                .default(true)
                .interact()?;
            if continue_project {
                run_project_init(&project_path).await?;
            }
        }
        (false, true) => {
            // Only user init needed; project already configured.
            run_user_init(&user_path).await?;
            println!("User configuration complete.");
        }
        (true, false) => {
            run_project_init(&project_path).await?;
        }
        (true, true) => {
            println!("Configuration already exists. Use `krew config add/del` to modify.");
        }
    }

    Ok(())
}

// ── User Init ───────────────────────────────────────────────────────

async fn run_user_init(user_path: &std::path::Path) -> anyhow::Result<()> {
    println!("\n=== User Configuration (Providers) ===\n");

    let mut count = 0;
    let mut existing_names: Vec<String> = Vec::new();

    loop {
        count += 1;
        if count > 1 {
            println!();
        }
        println!("Add provider [{}]", count);

        let data = collect_provider_data(&existing_names)?;
        krew_config::writer::add_provider(user_path, &data)?;

        println!(
            "Added provider \"{}\" ({})",
            data.name,
            provider_type_label(data.provider_type)
        );
        existing_names.push(data.name);

        let more = Confirm::new()
            .with_prompt("Add another provider?")
            .default(false)
            .interact()?;
        if !more {
            break;
        }
    }

    // Summary table.
    println!("\n--- Providers Summary ---");
    let providers = krew_config::writer::list_providers(user_path)?;
    print_provider_summary(&providers);
    println!();

    Ok(())
}

/// Collect data for a single provider via interactive prompts.
pub fn collect_provider_data(existing_names: &[String]) -> anyhow::Result<ProviderWriteData> {
    // 1. Select provider type.
    let type_labels = &["Anthropic", "OpenAI", "Google", "OpenAI-Compatible"];
    let type_idx = Select::new()
        .with_prompt("Select provider type")
        .items(type_labels)
        .default(0)
        .interact()?;

    let (provider_type, is_compatible) = match type_idx {
        0 => (ProviderType::Anthropic, false),
        1 => (ProviderType::OpenAI, false),
        2 => (ProviderType::Google, false),
        3 => (ProviderType::OpenAI, true),
        _ => unreachable!(),
    };

    // 2. Provider name with auto-suggestion.
    let base_name = match type_idx {
        0 => "anthropic",
        1 => "openai",
        2 => "google",
        3 => "openai-compatible",
        _ => "provider",
    };
    let suggested_name = unique_name(base_name, existing_names);

    let name: String = Input::new()
        .with_prompt("Provider name")
        .default(suggested_name)
        .validate_with(|input: &String| -> Result<(), String> {
            if input.is_empty() {
                return Err("Name cannot be empty".to_string());
            }
            if existing_names.contains(input) {
                return Err(format!(
                    "Provider name \"{}\" already exists, please enter a different name",
                    input
                ));
            }
            Ok(())
        })
        .interact_text()?;

    // 3. API key storage method.
    let key_methods = &["Environment variable", "Store in config file"];
    let key_idx = Select::new()
        .with_prompt("API key storage method")
        .items(key_methods)
        .default(0)
        .interact()?;

    let (api_key, api_key_env) = if key_idx == 0 {
        // Environment variable.
        let default_env = match type_idx {
            0 => "ANTHROPIC_API_KEY",
            1 => "OPENAI_API_KEY",
            2 => "GOOGLE_API_KEY",
            3 => "OPENAI_API_KEY",
            _ => "API_KEY",
        };
        let env_name: String = Input::new()
            .with_prompt("Environment variable name")
            .default(default_env.to_string())
            .interact_text()?;
        (None, Some(env_name))
    } else {
        let key: String = Password::new().with_prompt("API key").interact()?;
        (Some(key), None)
    };

    // 4. Base URL — required for OpenAI-Compatible, with defaults for others.
    let base_url = if is_compatible {
        let url: String = Input::new().with_prompt("Base URL").interact_text()?;
        Some(url)
    } else {
        let default_url = match type_idx {
            0 => "https://api.anthropic.com",
            1 => "https://api.openai.com",
            2 => "https://generativelanguage.googleapis.com/v1beta",
            _ => "",
        };
        let url: String = Input::new()
            .with_prompt("Base URL")
            .default(default_url.to_string())
            .interact_text()?;
        if url == default_url { None } else { Some(url) }
    };

    // 5. Google: Gemini API vs Vertex AI.
    let (vertex_project, vertex_location) = if provider_type == ProviderType::Google {
        let google_modes = &["Gemini API", "Vertex AI"];
        let mode_idx = Select::new()
            .with_prompt("Google API mode")
            .items(google_modes)
            .default(0)
            .interact()?;

        if mode_idx == 1 {
            let project: String = Input::new()
                .with_prompt("Vertex AI project ID")
                .interact_text()?;
            let location: String = Input::new()
                .with_prompt("Vertex AI location")
                .default("us-central1".to_string())
                .interact_text()?;
            (Some(project), Some(location))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };

    Ok(ProviderWriteData {
        name,
        provider_type,
        api_key,
        api_key_env,
        base_url,
        vertex_project,
        vertex_location,
    })
}

// ── Project Init ────────────────────────────────────────────────────

async fn run_project_init(project_path: &std::path::Path) -> anyhow::Result<()> {
    println!("\n=== Project Configuration (Agents) ===\n");

    // Get merged providers.
    let providers = get_merged_providers()?;
    if providers.is_empty() {
        println!("No providers configured. Run `krew config add provider` first.");
        return Ok(());
    }

    println!("Available providers:");
    for (name, cfg) in &providers {
        println!("  - {} ({})", name, provider_type_label(cfg.provider_type));
    }
    println!();

    // Choose creation mode.
    let modes = &["Smart Preset", "Manual Setup"];
    let mode_idx = Select::new()
        .with_prompt("Select setup mode")
        .items(modes)
        .default(0)
        .interact()?;

    let agents = if mode_idx == 0 {
        run_smart_preset(&providers).await?
    } else {
        run_manual_setup(&providers).await?
    };

    if agents.is_empty() {
        println!("No agents configured.");
        return Ok(());
    }

    // Write agents.
    krew_config::writer::batch_add_agents(project_path, &agents)?;

    // Show summary.
    println!("\n--- Agents Summary ---");
    print_agent_summary(&agents);
    let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
    println!("Reply order: {}", names.join(" -> "));
    println!("\nProject configuration saved to {}", CONFIG_FILENAME);

    Ok(())
}

// ── Smart Preset ────────────────────────────────────────────────────

async fn run_smart_preset(
    providers: &[(String, ProviderConfig)],
) -> anyhow::Result<Vec<AgentWriteData>> {
    println!("Fetching available models...");

    // Collect all (provider_name, model_id) candidates.
    let mut candidates: Vec<(String, String)> = Vec::new();

    for (name, cfg) in providers {
        let api_key = resolve_api_key(cfg);
        if api_key.is_empty() {
            continue;
        }
        let config = ListModelsConfig {
            provider_type: cfg.provider_type,
            base_url: cfg.base_url.clone(),
            api_key,
            vertex_project: cfg.vertex_project.clone(),
            vertex_location: cfg.vertex_location.clone(),
        };
        let models = match list_models(&config).await {
            Ok(m) if !m.is_empty() => m,
            _ => fallback_models(cfg.provider_type),
        };
        for m in models {
            candidates.push((name.clone(), m.id));
        }
    }

    if candidates.is_empty() {
        println!(
            "Failed to fetch available models. Check provider configuration or use manual creation."
        );
        return Ok(Vec::new());
    }

    // Determine available presets.
    let agent_count = if candidates.len() >= 3 {
        let presets = &["Single Agent", "Three Agents"];
        let idx = Select::new()
            .with_prompt("Select preset")
            .items(presets)
            .default(0)
            .interact()?;
        if idx == 0 { 1 } else { 3 }
    } else {
        println!(
            "Only {} model(s) available, using Single Agent preset.",
            candidates.len()
        );
        1
    };

    let mut agents = Vec::new();
    let mut used_names: Vec<String> = Vec::new();
    let mut selected_indices: Vec<usize> = Vec::new();
    let labels: Vec<String> = candidates
        .iter()
        .map(|(p, m)| format!("{} ({})", m, p))
        .collect();

    for i in 0..agent_count {
        if labels.is_empty() {
            break;
        }
        let prompt = if agent_count == 1 {
            "Select model".to_string()
        } else {
            format!("Select model for agent {}", i + 1)
        };

        let available: Vec<(usize, &String)> = labels
            .iter()
            .enumerate()
            .filter(|(idx, _)| !selected_indices.contains(idx))
            .collect();

        let selected_labels: Vec<&str> = available.iter().map(|(_, l)| l.as_str()).collect();
        let sel = FuzzySelect::new()
            .with_prompt(&prompt)
            .items(&selected_labels)
            .default(0)
            .interact()?;

        let actual_idx = available[sel].0;
        selected_indices.push(actual_idx);
        let (provider_name, model_id) = &candidates[actual_idx];

        // Look up provider config for api_type inference.
        let prov_cfg = providers
            .iter()
            .find(|(n, _)| n == provider_name)
            .map(|(_, c)| c);
        let prov_type = prov_cfg
            .map(|c| c.provider_type)
            .unwrap_or(ProviderType::OpenAI);
        let prov_base_url = prov_cfg.and_then(|c| c.base_url.as_deref());

        let agent_name = derive_agent_name(model_id, &used_names);
        let display = capitalize_first(&agent_name);
        let color = AGENT_COLORS.get(i).copied().unwrap_or("white").to_string();

        // Confirm thinking.
        let thinking = Confirm::new()
            .with_prompt(format!("Enable thinking for {}?", agent_name))
            .default(true)
            .interact()?;

        // For OpenAI-Compatible providers, ask user to choose api_type.
        let api_type = prompt_api_type_if_compatible(prov_type, prov_base_url)?;

        agents.push(AgentWriteData {
            name: agent_name.clone(),
            display_name: display,
            provider: provider_name.clone(),
            model: model_id.clone(),
            color,
            enable_thinking: thinking,
            enable_web_search: false,
            tools: true,
            api_type,
            system_prompt: None,
        });
        used_names.push(agent_name);

        // Remove selected candidate for subsequent picks.
        // Note: we just skip already-selected indices in future iterations.
    }

    // Show preview and confirm.
    println!("\n--- Preview ---");
    print_agent_summary(&agents);

    let confirm = Confirm::new()
        .with_prompt("Write this configuration?")
        .default(true)
        .interact()?;

    if !confirm {
        println!("Cancelled.");
        return Ok(Vec::new());
    }

    Ok(agents)
}

// ── Manual Setup ────────────────────────────────────────────────────

async fn run_manual_setup(
    providers: &[(String, ProviderConfig)],
) -> anyhow::Result<Vec<AgentWriteData>> {
    let mut agents = Vec::new();
    let mut used_names: Vec<String> = Vec::new();
    let mut used_colors: Vec<String> = Vec::new();
    let mut count = 0;

    loop {
        count += 1;
        if count > 1 {
            println!();
        }
        println!("Add agent [{}]", count);

        let data = collect_agent_data(providers, &used_names, &used_colors).await?;
        used_names.push(data.name.clone());
        used_colors.push(data.color.clone());
        agents.push(data);

        let more = Confirm::new()
            .with_prompt("Add another agent?")
            .default(false)
            .interact()?;
        if !more {
            break;
        }
    }

    Ok(agents)
}

/// Collect data for a single agent via interactive prompts.
pub async fn collect_agent_data(
    providers: &[(String, ProviderConfig)],
    existing_names: &[String],
    used_colors: &[String],
) -> anyhow::Result<AgentWriteData> {
    // 1. Select provider.
    let provider_labels: Vec<String> = providers
        .iter()
        .map(|(n, c)| format!("{} ({})", n, provider_type_label(c.provider_type)))
        .collect();
    let prov_idx = Select::new()
        .with_prompt("Select provider")
        .items(&provider_labels)
        .default(0)
        .interact()?;
    let (provider_name, provider_cfg) = &providers[prov_idx];

    // 2. Select/FuzzySelect model.
    let api_key = resolve_api_key(provider_cfg);
    let models = if !api_key.is_empty() {
        let config = ListModelsConfig {
            provider_type: provider_cfg.provider_type,
            base_url: provider_cfg.base_url.clone(),
            api_key,
            vertex_project: provider_cfg.vertex_project.clone(),
            vertex_location: provider_cfg.vertex_location.clone(),
        };
        match list_models(&config).await {
            Ok(m) if !m.is_empty() => m,
            _ => fallback_models(provider_cfg.provider_type),
        }
    } else {
        fallback_models(provider_cfg.provider_type)
    };

    let model_id = if models.is_empty() {
        Input::new().with_prompt("Model name").interact_text()?
    } else {
        let model_labels: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        let idx = FuzzySelect::new()
            .with_prompt("Select model")
            .items(&model_labels)
            .default(0)
            .interact()?;
        models[idx].id.clone()
    };

    // 3. Agent name.
    let suggested = derive_agent_name(&model_id, existing_names);
    let name: String = Input::new()
        .with_prompt("Agent name")
        .default(suggested)
        .validate_with(|input: &String| -> Result<(), String> {
            if input.is_empty() {
                return Err("Name cannot be empty".to_string());
            }
            if existing_names.contains(input) {
                return Err(format!("Agent name \"{}\" already exists", input));
            }
            Ok(())
        })
        .interact_text()?;

    // 4. Display name.
    let default_display = capitalize_first(&name);
    let display_name: String = Input::new()
        .with_prompt("Display name")
        .default(default_display)
        .interact_text()?;

    // 5. Color — unused colors first, then used colors (marked).
    let sorted_colors: Vec<String> = {
        let mut unused: Vec<String> = Vec::new();
        let mut used: Vec<String> = Vec::new();
        for &c in AGENT_COLORS {
            if used_colors.contains(&c.to_string()) {
                used.push(format!("{} (used)", c));
            } else {
                unused.push(c.to_string());
            }
        }
        unused.extend(used);
        unused
    };
    let color_idx = Select::new()
        .with_prompt("Select color")
        .items(&sorted_colors)
        .default(0)
        .interact()?;
    // Strip " (used)" suffix if present.
    let color = sorted_colors[color_idx]
        .strip_suffix(" (used)")
        .unwrap_or(&sorted_colors[color_idx])
        .to_string();

    // 6. Thinking & web search.
    let thinking = Confirm::new()
        .with_prompt("Enable thinking?")
        .default(true)
        .interact()?;
    let web_search = Confirm::new()
        .with_prompt("Enable web search?")
        .default(false)
        .interact()?;

    // For OpenAI-Compatible providers, ask user to choose api_type.
    let api_type = prompt_api_type_if_compatible(
        provider_cfg.provider_type,
        provider_cfg.base_url.as_deref(),
    )?;

    Ok(AgentWriteData {
        name,
        display_name,
        provider: provider_name.clone(),
        model: model_id.clone(),
        color,
        enable_thinking: thinking,
        enable_web_search: web_search,
        tools: true,
        api_type,
        system_prompt: None,
    })
}

// ── Helpers ─────────────────────────────────────────────────────────

/// Get merged providers from user + project config.
pub fn get_merged_providers() -> anyhow::Result<Vec<(String, ProviderConfig)>> {
    let user_path = user_config_path();
    let project_path = PathBuf::from(CONFIG_FILENAME);

    let mut providers: HashMap<String, ProviderConfig> = HashMap::new();

    // Load user providers.
    if let Some(ref path) = user_path
        && path.exists()
    {
        let user_providers = krew_config::writer::list_providers(path)?;
        for (name, cfg) in user_providers {
            providers.insert(name, cfg);
        }
    }

    // Load project providers (overrides user).
    if project_path.exists() {
        let proj_providers = krew_config::writer::list_providers(&project_path)?;
        for (name, cfg) in proj_providers {
            providers.insert(name, cfg);
        }
    }

    let mut result: Vec<(String, ProviderConfig)> = providers.into_iter().collect();
    result.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(result)
}

fn resolve_api_key(cfg: &ProviderConfig) -> String {
    if let Some(ref key) = cfg.api_key {
        return key.clone();
    }
    if let Some(ref env) = cfg.api_key_env
        && let Ok(val) = std::env::var(env)
    {
        return val;
    }
    String::new()
}

fn derive_agent_name(model_id: &str, existing: &[String]) -> String {
    // Extract prefix before first '-' that is followed by a version/number.
    let base = model_id.split('-').next().unwrap_or(model_id).to_string();

    unique_name(&base, existing)
}

fn unique_name(base: &str, existing: &[String]) -> String {
    if !existing.contains(&base.to_string()) {
        return base.to_string();
    }
    for i in 2.. {
        let name = format!("{}-{}", base, i);
        if !existing.contains(&name) {
            return name;
        }
    }
    unreachable!()
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// For OpenAI-Compatible providers (custom base_url), prompt the user to choose api_type.
/// For official OpenAI, default to "responses". For non-OpenAI providers, return None.
fn prompt_api_type_if_compatible(
    provider_type: ProviderType,
    base_url: Option<&str>,
) -> anyhow::Result<Option<String>> {
    if provider_type != ProviderType::OpenAI {
        return Ok(None);
    }

    let is_official = base_url
        .map(|u| u.contains("api.openai.com"))
        .unwrap_or(true);

    if is_official {
        return Ok(Some("responses".to_string()));
    }

    // OpenAI-Compatible: let user choose.
    let options = &["Chat Completions (recommended)", "Responses API"];
    let idx = Select::new()
        .with_prompt("Select API type")
        .items(options)
        .default(0)
        .interact()?;

    Ok(Some(
        if idx == 0 { "chat" } else { "responses" }.to_string(),
    ))
}

fn provider_type_label(t: ProviderType) -> &'static str {
    match t {
        ProviderType::OpenAI => "OpenAI",
        ProviderType::Anthropic => "Anthropic",
        ProviderType::Google => "Google",
    }
}

fn print_provider_summary(providers: &[(String, ProviderConfig)]) {
    println!(
        "{:<16} {:<12} {:<24} Base URL",
        "Name", "Type", "Key Method"
    );
    println!("{}", "-".repeat(72));
    for (name, cfg) in providers {
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
            "{:<16} {:<12} {:<24} {}",
            name,
            provider_type_label(cfg.provider_type),
            key_method,
            base
        );
    }
}

fn print_agent_summary(agents: &[AgentWriteData]) {
    println!(
        "{:<12} {:<12} {:<12} {:<24} {:<10} {:<9} Web",
        "Name", "Display", "Provider", "Model", "Color", "Thinking"
    );
    println!("{}", "-".repeat(90));
    for a in agents {
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
}
