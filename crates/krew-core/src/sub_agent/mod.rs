//! Sub-Agent discovery, definition types, and the `run_agent` tool.

mod discovery;
mod run_agent_tool;
mod types;

pub use discovery::{build_sub_agent_catalog, discover_sub_agents};
pub use run_agent_tool::RunAgentTool;
pub use types::SubAgentDef;
