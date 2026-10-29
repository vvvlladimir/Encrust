use serde::{Deserialize, Serialize};

/// The edge of the plate, in plate axes, that faces someone standing at the printer. A panel
/// mounted portrait on a machine wider than deep puts it on a long side. See ADR 0369.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrontEdge {
    #[default]
    MinY,
    MinX,
    MaxX,
}

impl FrontEdge {
    /// The turn about Z, radians counter-clockwise from above, carrying the `MinY` edge here.
    pub fn turn_rad(self) -> f32 {
        match self {
            Self::MinY => 0.0,
            Self::MinX => -std::f32::consts::FRAC_PI_2,
            Self::MaxX => std::f32::consts::FRAC_PI_2,
        }
    }

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "serde's skip_serializing_if passes a reference"
    )]
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::MinY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_edge_reads_from_its_snake_case_name() {
        #[derive(Deserialize)]
        struct Row {
            front_edge: FrontEdge,
        }
        for (text, edge) in [
            ("min_y", FrontEdge::MinY),
            ("min_x", FrontEdge::MinX),
            ("max_x", FrontEdge::MaxX),
        ] {
            let row: Row =
                toml::from_str(&format!("front_edge = \"{text}\"")).expect("a known edge");
            assert_eq!(row.front_edge, edge);
        }
    }
}
