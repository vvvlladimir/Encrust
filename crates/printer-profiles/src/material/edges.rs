use std::num::NonZeroU8;

use serde::{Deserialize, Serialize};

/// How a mask's edge is shaded: a resin's default, and what a plate cuts with (ADR 0380).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EdgeSmoothing {
    /// Edge pixels are lit by how much of them the model covers; off, fully or not at all.
    pub anti_alias: bool,
    /// The greys an anti-aliased edge is rounded to, or `None` for every grey the panel takes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grey_levels: Option<NonZeroU8>,
    /// Radius an anti-aliased edge is faded over, px; `0` leaves it sharp.
    pub blur_px: u8,
}

impl Default for EdgeSmoothing {
    fn default() -> Self {
        Self {
            anti_alias: true,
            grey_levels: None,
            blur_px: 0,
        }
    }
}
