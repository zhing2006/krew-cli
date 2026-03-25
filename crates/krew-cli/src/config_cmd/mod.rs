mod add;
mod del;
mod doctor;
mod help;
mod init;
mod list;

use crate::ConfigAction;

/// Dispatch a config subcommand action.
pub async fn dispatch(action: ConfigAction) -> i32 {
    let result = match action {
        ConfigAction::Init { user, project } => init::run(user, project).await,
        ConfigAction::Add { target } => add::run(target).await,
        ConfigAction::Del { target } => del::run(target).await,
        ConfigAction::List { target } => list::run(target).await,
        ConfigAction::Doctor => doctor::run().await,
        ConfigAction::Help => help::run().await,
    };
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e}");
            1
        }
    }
}
