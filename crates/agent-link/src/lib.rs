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
pub use places::{
    AGENTS, JOBS, RevisionFile, WINDOW_FILE, cache_dir, is_job_name, latest_revision, revision_of,
};
pub use presence::{AGENT_SCHEMA, AgentFile, Working};
