//! Core logic for krew-cli: session management, agent loop, message routing,
//! and slash commands.

pub mod agent;
pub mod command;
pub mod compact;
pub mod custom_command;
pub mod discovery;
pub mod dream;
pub mod event;
pub mod memory;
pub mod persistence;
pub mod process_stats;
pub mod router;
pub mod skill;
pub mod sub_agent;
