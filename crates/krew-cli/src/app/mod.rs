mod agent_display;
pub mod approval;
mod commands;
mod input;
mod message;
mod paste_burst;
mod persistence;
mod state;

pub use state::App;
use state::QUIT_SHORTCUT_TIMEOUT;
