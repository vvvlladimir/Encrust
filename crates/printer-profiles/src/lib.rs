//! Printer hardware descriptions, material exposure settings and support shapes, stored
//! as TOML.

mod catalogue;
mod compensation;
mod error;
mod exposure;
mod material;
mod printer;
mod store;
mod support;

pub use catalogue::{Catalogue, Entry, Kind, PROFILE_DIR_VAR, Source, is_valid_id, user_dir};
pub use compensation::{Compensation, layer_time_between, shrink_pct_between};
pub use error::ProfileError;
pub use exposure::{ASSUMED_PENETRATION_DEPTH_MM, exposure_for_mm};
pub use material::{MaterialProfile, PriceUnit, PrinterTuning, ResinDetails, WaitMode, Waits};
pub use printer::{
    AnycubicExtension, BuildVolume, Connection, Display, Firmware, OutputFormat, PhotonRevision,
    PrinterProfile,
};
pub use store::{DirStore, ProfileStore};
pub use support::{
    BottomSegment, Bracing, Branching, ContactShape, MiddleSegment, PlatformShape, Raft, RaftShape,
    SmallPillar, SupportProfile, TipSegment, TopSegment,
};
