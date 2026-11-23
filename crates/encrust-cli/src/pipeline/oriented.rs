//! What an orientation search chose, as a run reports it.

use core_document::{degrees_of, supported_settings};
use core_engine::Printout;
use core_geometry::{Quat, Scalar, Vec3};
use core_plate::OrientSettings;
use printer_profiles::SupportProfile;

/// The turn auto-orientation applied, and why it chose it.
pub struct OrientSummary {
    pub rotation: Quat,
    pub peak_mm2: Scalar,
    pub overhang_mm2: Scalar,
    /// Whether the model needs no supports this way up, set straight on the plate.
    pub on_plate: bool,
    /// The search's best ways to stand it, as many as `--orient-choices` asks for.
    pub choices: Vec<OrientChoice>,
}

/// One way the search offers to stand the model, and what it printed when tried.
pub struct OrientChoice {
    pub rotation: Quat,
    pub on_plate: bool,
    /// Whether it is worth offering beside the first.
    pub offered: bool,
    pub printout: Option<Printout>,
}

/// The angles `--rotate X,Y,Z` repeats `rotation` with, degrees. Zero is added to each, so
/// float dust never reads as a turn of minus nothing.
pub fn rotate_degrees(rotation: Quat) -> Vec3 {
    degrees_of(rotation)
}

impl OrientSummary {
    /// How far the turn goes, degrees.
    pub fn degrees(&self) -> Scalar {
        self.rotation.to_axis_angle().1.to_degrees()
    }

    /// The angles `--rotate X,Y,Z` repeats the turn with, degrees.
    pub fn rotate_degrees(&self) -> Vec3 {
        rotate_degrees(self.rotation)
    }

    /// The turn in words: its axis where that is one of the plate's own, and the flag that
    /// repeats it.
    pub(crate) fn describe(&self) -> String {
        let flag = {
            let turn = self.rotate_degrees();
            format!("--rotate {:.0},{:.0},{:.0}", turn.x, turn.y, turn.z)
        };
        match self.axis_name() {
            Some(axis) => format!(
                "turned {:.0} degrees around {axis} ({flag})",
                self.degrees()
            ),
            None => format!("turned {:.0} degrees ({flag})", self.degrees()),
        }
    }

    /// X, Y or Z when the turn is about that axis of the plate, `None` for any other.
    pub(super) fn axis_name(&self) -> Option<&'static str> {
        let axis = self.rotation.to_axis_angle().0;
        [("X", Vec3::X), ("Y", Vec3::Y), ("Z", Vec3::Z)]
            .into_iter()
            .find(|(_, about)| axis.dot(*about).abs() > 1.0 - 1.0e-3)
            .map(|(name, _)| name)
    }
}

/// How the search judges a model that will stand on `supports`, or on the plate.
pub fn orient_settings(supports: Option<&SupportProfile>) -> OrientSettings {
    supports.map_or_else(OrientSettings::default, supported_settings)
}
