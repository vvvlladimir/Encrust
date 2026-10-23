//! Supports from the command line: found on the model's own stack, meshed, and merged
//! into what gets sliced.
//!
//! The same sequence the window runs, without the hand placement: cut the model, ask
//! where it needs holding, drop a column under each contact, merge the near ones into
//! trunks and mesh the lot. See `docs/design/supports.md`.

use std::fmt;

use anyhow::{Context, Result};
use core_geometry::{Mesh, Scalar, Vec3};
use printer_profiles::SupportProfile;

/// What automatic placement put under one model.
pub struct SupportReport {
    /// How far the model was stood off the plate to make room, millimetres; zero for one
    /// that already stood clear of it.
    pub lifted_mm: Scalar,
    /// Where each contact touches the model, plate coordinates, millimetres.
    pub contacts: Vec<Vec3>,
    pub standing: usize,
    pub trees: usize,
    pub faces: usize,
}

impl SupportReport {
    /// Contacts the run found but could not stand a column under.
    pub fn unsupported(&self) -> usize {
        self.contacts.len().saturating_sub(self.standing)
    }
}

impl fmt::Display for SupportReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Supports")?;
        if self.lifted_mm > 0.0 {
            writeln!(f, "  lifted         {:.2} mm", self.lifted_mm)?;
        }
        writeln!(f, "  contacts       {}", self.contacts.len())?;
        writeln!(f, "  standing       {}", self.standing)?;
        writeln!(f, "  trunks         {}", self.trees)?;
        writeln!(f, "  triangles      {}", self.faces)?;
        if self.unsupported() > 0 {
            writeln!(f, "  no room for    {}", self.unsupported())?;
        }
        writeln!(f)
    }
}

/// Stands supports under `mesh`, in plate coordinates, and answers with them: the
/// engine's `stand_under`, or `stand_seated` for a model left on the plate, with what it
/// found laid out for a terminal.
pub fn stand_under(
    mesh: &mut Mesh,
    cuts: &mut Option<Mesh>,
    profile: &SupportProfile,
    layer_height_mm: Scalar,
    seated: bool,
) -> Result<(SupportReport, Mesh)> {
    let stand = if seated {
        core_engine::stand_seated
    } else {
        core_engine::stand_under
    };
    let stood = stand(mesh, cuts, profile, layer_height_mm)
        .context("cannot stand supports under the model")?;
    let report = SupportReport {
        lifted_mm: stood.lifted_mm,
        contacts: stood.contacts,
        standing: stood.standing,
        trees: stood.trees,
        faces: stood.supports.faces.len(),
    };
    Ok((report, stood.supports))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat slab hanging ten millimetres over the plate: nothing holds it up, so
    /// placement has to put something under it.
    fn overhang() -> Mesh {
        let z = 10.0;
        let corners = [
            Vec3::new(0.0, 0.0, z),
            Vec3::new(20.0, 0.0, z),
            Vec3::new(20.0, 20.0, z),
            Vec3::new(0.0, 20.0, z),
            Vec3::new(0.0, 0.0, z + 2.0),
            Vec3::new(20.0, 0.0, z + 2.0),
            Vec3::new(20.0, 20.0, z + 2.0),
            Vec3::new(0.0, 20.0, z + 2.0),
        ];
        Mesh::new(
            corners.to_vec(),
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
        )
    }

    #[test]
    fn an_overhang_gets_columns_under_it_and_they_come_back_as_a_mesh() {
        let mut mesh = overhang();
        let (report, supports) =
            stand_under(&mut mesh, &mut None, &SupportProfile::shipped("medium").expect("the medium preset ships"), 0.05, false)
                .expect("a slab slices and supports");

        assert!(!report.contacts.is_empty(), "a hanging slab needs holding");
        assert!(report.standing > 0, "and there is empty plate under it");
        assert_eq!(
            supports.faces.len(),
            report.faces,
            "the columns come back to be sliced with the model"
        );
    }

    /// A 20 mm slab lying on the plate, which is where `--center` leaves a model.
    fn slab_on_the_plate() -> Mesh {
        Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Vec3::new(20.0, 20.0, 0.0),
                Vec3::new(0.0, 20.0, 0.0),
                Vec3::new(0.0, 0.0, 2.0),
                Vec3::new(20.0, 0.0, 2.0),
                Vec3::new(20.0, 20.0, 2.0),
                Vec3::new(0.0, 20.0, 2.0),
            ],
            vec![
                [0, 2, 1],
                [0, 3, 2],
                [4, 5, 6],
                [4, 6, 7],
                [0, 1, 5],
                [0, 5, 4],
                [1, 2, 6],
                [1, 6, 5],
                [2, 3, 7],
                [2, 7, 6],
                [3, 0, 4],
                [3, 4, 7],
            ],
        )
    }

    #[test]
    fn a_slab_on_the_plate_is_lifted_before_anything_is_stood_under_it() {
        let mut mesh = slab_on_the_plate();
        let profile = SupportProfile::shipped("medium").expect("the medium preset ships");

        let (report, _) = stand_under(&mut mesh, &mut None, &profile, 0.05, false)
            .expect("a slab slices and supports");

        assert!(
            (report.lifted_mm - profile.z_lift_mm).abs() < 1e-4,
            "expected the profile's own {} mm of lift, got {}",
            profile.z_lift_mm,
            report.lifted_mm
        );
        let lowest = mesh.vertices[..8]
            .iter()
            .fold(Scalar::MAX, |low, corner| low.min(corner.z));
        assert!(
            (lowest - profile.z_lift_mm).abs() < 1e-4,
            "the lowest point of the part stands at the lift, got {lowest}"
        );
        assert!(
            report.standing > 0,
            "a lifted slab has columns under it, not an empty report"
        );
    }

    #[test]
    fn a_part_already_standing_clear_of_the_plate_is_not_moved() {
        let mut mesh = overhang();
        let (report, _) = stand_under(&mut mesh, &mut None, &SupportProfile::shipped("medium").expect("the medium preset ships"), 0.05, false)
            .expect("a slab slices and supports");

        assert!(
            report.lifted_mm.abs() < 1e-6,
            "a slab 10 mm up has room already, got a lift of {}",
            report.lifted_mm
        );
    }
}
