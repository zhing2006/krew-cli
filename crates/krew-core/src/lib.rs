//! Core logic for krew-cli: session management, agent loop, message routing,
//! and slash commands.

pub mod agent;
pub mod command;
pub mod compact;
pub mod event;
pub mod persistence;
pub mod process_stats;
pub mod router;
pub mod skill;
