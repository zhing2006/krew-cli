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
            // Ctrl-C during interactive prompts — exit silently.
            if is_user_interrupt(&e) {
                eprintln!();
                return 130;
            }
            eprintln!("Error: {e}");
            1
        }
    }
}

/// Check if an error chain contains a user interrupt (Ctrl-C / broken pipe).
fn is_user_interrupt(err: &anyhow::Error) -> bool {
    for cause in err.chain() {
        if let Some(io_err) = cause.downcast_ref::<std::io::Error>() {
            match io_err.kind() {
                std::io::ErrorKind::Interrupted => return true,
                // dialoguer on Windows uses "operation was canceled" (ErrorKind::Other)
                // with the raw_os_error 995 (ERROR_OPERATION_ABORTED).
                _ if io_err.raw_os_error() == Some(995) => return true,
                _ => {}
            }
        }
        // dialoguer wraps Ctrl-C as a plain message containing "interrupted".
        let msg = cause.to_string().to_lowercase();
        if msg.contains("interrupted") || msg.contains("cancelled") {
            return true;
        }
    }
    false
}
