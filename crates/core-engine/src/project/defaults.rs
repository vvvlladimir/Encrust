//! What a plate starts at: the numbers a fresh window's tools hold before anything is set.

use core_volume::{HoleSize, HollowSettings, InfillSettings, PartSettings};
use printer_profiles::SupportProfile;

use super::{
    Array, Axis, CutState, DrainState, FillProfiles, Group, HollowState, Keep, Manifest,
    SlicingState, SupportState, VERSION,
};

/// What the one support group a plate starts with is called.
pub const FIRST_GROUP: &str = "Default";

/// What the first plate of a project is called.
pub const FIRST_PLATE: &str = "Plate 1";

impl Default for HollowState {
    fn default() -> Self {
        let asked = HollowSettings::default();
        Self {
            thickness_mm: asked.thickness_mm,
            mode: asked.mode,
            precision: asked.precision,
            infill_on: false,
            infill: InfillSettings::default(),
            open_floor: false,
            solid_under_mm: asked.solid_under_mm,
        }
    }
}

impl Default for DrainState {
    fn default() -> Self {
        let hole = HoleSize::default();
        Self::round(hole.diameter_mm, hole.depth_mm, hole.taper)
    }
}

impl Default for CutState {
    fn default() -> Self {
        Self {
            axis: Axis::Z,
            height_mm: 10.0,
            keep: Keep::Both,
            joints: core_joint::Joints::default(),
            parts: PartSettings::default(),
        }
    }
}

impl Default for SupportState {
    fn default() -> Self {
        let profile = SupportProfile::default();
        Self {
            raft: Some(profile.raft),
            groups: vec![Group {
                name: FIRST_GROUP.to_owned(),
                profile,
            }],
            active: 0,
            fill: core_supports::ProjectSettings::default(),
            brush_radius_mm: 2.0,
            flood_angle_deg: 30.0,
            inline_spacing_mm: core_supports::INLINE_SPACING_MM,
            fill_profiles: FillProfiles::default(),
            sizing: None,
        }
    }
}

impl Manifest {
    /// An empty project on the named printer and resin: the default tools, one plate, no objects.
    pub fn new(printer: Option<String>, resin: Option<String>) -> Self {
        Self {
            version: VERSION,
            printer,
            resin,
            slicing: SlicingState::default(),
            supports: SupportState::default(),
            hollow: HollowState::default(),
            drain: DrainState::default(),
            cut: CutState::default(),
            array: Array::default(),
            plates: vec![FIRST_PLATE.to_owned()],
            active_plate: 0,
            objects: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::super::{Project, read_from, write_to};
    use super::*;

    #[test]
    fn a_new_manifest_comes_back_equal_in_every_field() {
        let saved = Project {
            manifest: Manifest::new(Some("a-printer".to_owned()), Some("a-resin".to_owned())),
            models: Vec::new(),
            preview: None,
        };
        let mut bytes = Cursor::new(Vec::new());
        write_to(&mut bytes, &saved).expect("an empty project in memory writes");
        bytes.set_position(0);
        let back = read_from(bytes)
            .expect("what was just written reads")
            .manifest;
        let was = saved.manifest;

        assert_eq!(back.version, VERSION);
        assert_eq!(back.printer, was.printer);
        assert_eq!(back.resin, was.resin);
        assert_eq!(back.slicing, was.slicing);
        assert_eq!(back.supports, was.supports);
        assert_eq!(back.hollow, was.hollow);
        assert_eq!(back.drain, was.drain);
        assert_eq!(back.cut, was.cut);
        assert_eq!(back.array, was.array);
        assert_eq!(back.plates, [FIRST_PLATE]);
        assert_eq!(back.active_plate, 0);
        assert!(back.objects.is_empty());
    }
}
