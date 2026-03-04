//! Tool approval overlay for interactive approve/deny workflow.
//!
//! Simplified for krew-cli's inline viewport architecture:
//! - No MCP elicitation, network approval, or multi-thread routing
//! - Renders directly into the viewport area
//! - Uses oneshot channel to send decision back to agent loop

mod overlay;

pub use overlay::ApprovalOverlay;
