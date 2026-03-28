use krew_core::command::SlashCommand;

#[test]
fn from_input_parses_tools() {
    assert!(matches!(
        SlashCommand::from_input("/tools"),
        Some(SlashCommand::Tools)
    ));
}

#[test]
fn from_input_unknown_returns_none() {
    assert!(SlashCommand::from_input("/unknown").is_none());
}

#[test]
fn tools_name_matches() {
    assert_eq!(SlashCommand::Tools.name(), "/tools");
}

#[test]
fn all_help_contains_tools() {
    let entries = SlashCommand::all_help();
    assert!(entries.iter().any(|(name, _)| *name == "/tools"));
}

#[test]
fn all_help_entries_round_trip_through_from_input() {
    // Every command listed in all_help() should parse successfully.
    for &(name, _) in SlashCommand::all_help() {
        assert!(
            SlashCommand::from_input(name).is_some(),
            "{name} listed in all_help() but not parseable"
        );
    }
}
