//! Printer hardware descriptions, material exposure settings, support shapes and named
//! tool settings, stored as TOML.

mod catalogue;
mod compensation;
mod error;
mod exchange;
mod exposure;
mod material;
mod own_ids;
mod preset;
mod printer;
mod store;
mod support;

pub use catalogue::{Catalogue, Entry, Kind, PROFILE_DIR_VAR, Source, is_valid_id, user_dir};
pub use compensation::{Compensation, layer_time_between, shrink_pct_between};
pub use error::ProfileError;
pub use exchange::unique_id;
pub use exposure::{ASSUMED_PENETRATION_DEPTH_MM, exposure_for_mm};
pub use material::{MaterialProfile, PriceUnit, PrinterTuning, ResinDetails, WaitMode, Waits};
pub use own_ids::{Renamed, own_id};
pub use preset::Preset;
pub use printer::{
    AnycubicExtension, BuildVolume, Connection, Display, Firmware, OutputFormat, PhotonRevision,
    PrinterProfile,
};
pub use store::{DirStore, ProfileStore};
pub use support::{
    BottomSegment, Bracing, Branching, ContactShape, MiddleSegment, PlatformShape, Raft, RaftShape,
    RaftStyle, SmallPillar, SupportProfile, TipSegment, TopSegment,
};
