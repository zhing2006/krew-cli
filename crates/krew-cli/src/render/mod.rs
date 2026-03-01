mod header;
pub mod highlight;
pub mod markdown;
mod messages;
mod popup;
mod viewport;

pub use header::insert_header;
pub use messages::insert_lines;
pub use viewport::{parse_color, render_input_viewport};
