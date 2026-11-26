//! What `encrust-mcp` and the open window say to each other, and where they keep it: the lines
//! of the window link, the file each agent keeps and the folders under the user's cache
//! (ADR 0486, `docs/design/window-link.md`).

mod line;
mod places;
mod presence;

pub use line::{
    ApplyReply, HandedPlate, Header, PLATE_SCHEMA, PlateLine, SHOW_SCHEMA, Show, ShowReply, Signed,
    WindowFile, plate_refused,
};
pub use places::{AGENTS, JOBS, WINDOW_FILE, cache_dir};
pub use presence::{AGENT_SCHEMA, AgentFile, Working};
