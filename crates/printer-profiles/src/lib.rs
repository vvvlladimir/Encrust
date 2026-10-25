//! Printer hardware descriptions, material exposure settings, support shapes and named
//! tool settings, stored as TOML.

mod auto;
mod catalogue;
mod compensation;
mod error;
mod exchange;
mod exposure;
mod foreign;
mod material;
mod own_ids;
mod preset;
mod printer;
mod stated;
mod store;
mod support;

pub use auto::{AUTO_SIZES, AutoProfile};
pub use catalogue::{
    Catalogue, Entry, Kind, PROFILE_DIR_VAR, Source, Unreadable, is_valid_id, user_dir,
};
pub use compensation::{Compensation, layer_time_between, shrink_pct_between};
pub use error::ProfileError;
pub use exchange::unique_id;
pub use exposure::{ASSUMED_PENETRATION_DEPTH_MM, exposure_for_mm};
pub use foreign::ForeignResin;
pub use material::{MaterialProfile, PriceUnit, PrinterTuning, ResinDetails, WaitMode, Waits};
pub use own_ids::{Renamed, own_id};
pub use preset::Preset;
pub use printer::{
    AnycubicExtension, BuildVolume, Connection, Display, Firmware, NetworkMachine, OutputFormat,
    PhotonRevision, PrinterProfile,
};
pub use stated::{
    stated_distance_mm, stated_pwm, stated_rests, stated_speed_mm_min, stated_wait_s,
};
pub use store::{DirStore, ProfileStore};
pub use support::{
    BottomSegment, BracePattern, BraceZone, Bracing, BracingPreset, Branching, ContactShape, Grid,
    IslandSpacing, MiddleSegment, PlatformShape, Raft, RaftShape, RaftStyle, STOCK_SUPPORT,
    SmallPillar, SupportProfile, TipSegment, TopSegment,
};
