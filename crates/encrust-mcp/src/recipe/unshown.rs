//! What a recipe asks that no step of a `core-recipe` file says yet, so the window running the
//! revision's steps does not do it (ADR 0459).

use super::{Drains, Hollow, Pose, Recipe, Repair, Supports};

impl Recipe {
    /// Each thing this recipe asks that the window leaves out, in words for the client.
    pub fn unshown(&self) -> Vec<&'static str> {
        let mut left = Vec::new();
        let mut note = |asked: bool, what: &'static str| {
            if asked {
                left.push(what);
            }
        };
        let repair_mode = matches!(self.repair, Some(Repair::CloseSmallHoles | Repair::Turn));
        note(
            repair_mode,
            "a repair of small holes or of turned faces alone",
        );
        note(
            self.mend.is_some(),
            "faces turned, bridged or dropped by hand",
        );
        note(
            self.modelling.is_some(),
            "the scale, combine, label, cut, split or arrange",
        );
        note(self.relief.is_some(), "the relief");
        note(self.slicing.is_some(), "the slicing settings");
        if let Some(pose) = &self.pose {
            pose_unshown(pose, &mut note);
        }
        if let Some(hollow) = &self.hollow {
            hollow_unshown(hollow, &mut note);
        }
        if let Some(drains) = &self.drains {
            drains_unshown(drains, &mut note);
        }
        if let Some(Supports {
            at_mm, shown_at_mm, ..
        }) = &self.supports
        {
            note(!at_mm.is_empty(), "supports placed by hand at points");
            note(!shown_at_mm.is_empty(), "the side painted to be seen");
        }
        left
    }
}

fn pose_unshown(pose: &Pose, note: &mut impl FnMut(bool, &'static str)) {
    match pose {
        Pose::Rotate { .. } => note(true, "a turn given in degrees"),
        Pose::Search {
            pick,
            keep_clear_at_mm,
            ..
        } => {
            note(
                pick.is_some_and(|pick| pick > 1),
                "a pose picked from the search's list",
            );
            note(
                !keep_clear_at_mm.is_empty(),
                "places kept clear of supports",
            );
        }
    }
}

fn hollow_unshown(hollow: &Hollow, note: &mut impl FnMut(bool, &'static str)) {
    note(hollow.precision.is_some(), "the hollow's precision");
    note(
        !hollow.keep_solid_at_mm.is_empty(),
        "walls kept solid at points",
    );
    let pattern = hollow.infill_cell_mm.is_some() || hollow.infill_density.is_some();
    note(pattern, "the infill's cell and density");
}

fn drains_unshown(drains: &Drains, note: &mut impl FnMut(bool, &'static str)) {
    note(!drains.at_mm.is_empty(), "holes drilled at points");
    let shaped = drains.taper.is_some()
        || drains.sides.is_some()
        || drains.turn_deg.is_some()
        || drains.plug_gap_mm.is_some()
        || drains.kind.is_some();
    note(shaped, "a hole's shape, walled holes and plug gap");
    let elsewhere = !drains.support_at_mm.is_empty() || !drains.channels.is_empty();
    note(elsewhere, "holes into the supports and channels");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bare() -> Recipe {
        Recipe::bare("/m/cube.stl".into(), "p".into(), "r".into())
    }

    #[test]
    fn a_recipe_of_steps_alone_leaves_nothing_out() {
        let recipe = Recipe {
            prepare: true,
            hollow: Some(Hollow {
                wall_mm: 2.0,
                ..Hollow::default()
            }),
            ..bare()
        };
        assert!(recipe.unshown().is_empty());
    }

    #[test]
    fn points_and_a_picked_pose_are_named() {
        let recipe = Recipe {
            pose: Some(Pose::Search {
                goal: super::super::Goal::Auto,
                max_tilt_deg: None,
                pick: Some(2),
                keep_clear_at_mm: vec![[1.0, 2.0, 3.0]],
                keep_clear_radius_mm: None,
                keep_clear_seen_deg: None,
            }),
            supports: Some(Supports {
                profile: "auto".into(),
                at_mm: vec![[0.0; 3]],
                ..Supports::default()
            }),
            ..bare()
        };
        assert_eq!(
            recipe.unshown(),
            [
                "a pose picked from the search's list",
                "places kept clear of supports",
                "supports placed by hand at points"
            ]
        );
    }
}
