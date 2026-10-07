//! Supports from the command line: found on the model's own stack, meshed, and merged
//! into what gets sliced.
//!
//! The same sequence the window runs, without the hand placement: cut the model, ask
//! where it needs holding, drop a column under each contact, merge the near ones into
//! trunks and mesh the lot. See `docs/design/supports.md`.

use std::fmt;

use anyhow::{Context, Result};
use core_geometry::{Bvh, Mesh, Scalar, Transform, lift_over_plate, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings};
use core_supports::{Placed, Profiles, SupportPoint, columns, grow, mesh_trees};
use printer_profiles::SupportProfile;

/// What automatic placement put under one model.
pub struct SupportReport {
    /// How far the model was stood off the plate to make room, millimetres; zero for one
    /// that already stood clear of it.
    pub lifted_mm: Scalar,
    pub contacts: usize,
    pub standing: usize,
    pub trees: usize,
    pub faces: usize,
}

impl SupportReport {
    /// Contacts the run found but could not stand a column under.
    pub fn unsupported(&self) -> usize {
        self.contacts.saturating_sub(self.standing)
    }
}

impl fmt::Display for SupportReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Supports")?;
        if self.lifted_mm > 0.0 {
            writeln!(f, "  lifted         {:.2} mm", self.lifted_mm)?;
        }
        writeln!(f, "  contacts       {}", self.contacts)?;
        writeln!(f, "  standing       {}", self.standing)?;
        writeln!(f, "  trunks         {}", self.trees)?;
        writeln!(f, "  triangles      {}", self.faces)?;
        if self.unsupported() > 0 {
            writeln!(f, "  no room for    {}", self.unsupported())?;
        }
        writeln!(f)
    }
}

/// Stands supports under `mesh`, in plate coordinates, and appends them to it.
///
/// The model is lifted off the plate first, by the profile's own `z_lift_mm`: a part
/// sitting on the plate has no room under it for a support, and the command line has no
/// Lift button to press. See `docs/design/supports.md`.
///
/// The model is cut once here to find the contacts, and again by the caller to write the
/// file: the second pass is over the model and its supports together, which is the only
/// way what is printed matches what was measured.
pub fn stand_under(
    mesh: &mut Mesh,
    profile: &SupportProfile,
    layer_height_mm: Scalar,
) -> Result<SupportReport> {
    let lifted_mm = lift(mesh, profile.z_lift_mm)?;
    let stack = PlaneSliceEngine
        .slice(
            mesh,
            &SliceSettings {
                layer_height: layer_height_mm,
                ..SliceSettings::default()
            },
        )
        .context("cannot slice the model to work out where it needs supports")?;

    let contacts =
        core_supports::generate_supports(&stack, layer_height_mm, profile, &[], None, &mut |_| {
            true
        });
    let points: Vec<SupportPoint> = contacts.iter().copied().map(SupportPoint::new).collect();

    let bvh = Bvh::build(mesh);
    let placed = Placed::new(mesh, &bvh, Transform::default());
    let table = std::slice::from_ref(profile);
    let profiles = Profiles::new(table).context("a support profile is needed to build one")?;

    let columns = columns(&points, &placed, profiles);
    let trees = grow(&columns, &placed, profiles);
    let standing = trees
        .iter()
        .map(core_supports::SupportTree::tip_count)
        .sum();
    let supports = mesh_trees(&trees, &placed, profiles);

    let report = SupportReport {
        lifted_mm,
        contacts: points.len(),
        standing,
        trees: trees.len(),
        faces: supports.faces.len(),
    };
    append(mesh, &supports);
    Ok(report)
}

/// Stands `mesh` `lift_mm` clear of the plate and answers how far it moved. A model
/// already standing at least that high is left where it is: the lift makes room under a
/// part, it does not place it.
fn lift(mesh: &mut Mesh, lift_mm: Scalar) -> Result<Scalar> {
    let bounds = mesh.aabb().context("mesh has no vertices")?;
    if bounds.mins.z >= lift_mm {
        return Ok(0.0);
    }
    let offset = lift_over_plate(&bounds, lift_mm);
    *mesh = transform_mesh(mesh, Transform::from_translation(offset));
    Ok(offset.z)
}

/// Adds `other`'s geometry to `mesh`, keeping each face pointing at its own vertices.
fn append(mesh: &mut Mesh, other: &Mesh) {
    let offset = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&other.vertices);
    mesh.faces.extend(
        other
            .faces
            .iter()
            .map(|face| [face[0] + offset, face[1] + offset, face[2] + offset]),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::Vec3;

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
    fn an_overhang_gets_columns_under_it_and_they_go_into_the_mesh() {
        let mut mesh = overhang();
        let faces_before = mesh.faces.len();

        let report = stand_under(&mut mesh, &SupportProfile::medium(), 0.05)
            .expect("a slab slices and supports");

        assert!(report.contacts > 0, "a hanging slab needs holding");
        assert!(report.standing > 0, "and there is empty plate under it");
        assert_eq!(
            mesh.faces.len(),
            faces_before + report.faces,
            "the columns are part of what gets sliced"
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
        let profile = SupportProfile::medium();

        let report = stand_under(&mut mesh, &profile, 0.05).expect("a slab slices and supports");

        assert!(
            (report.lifted_mm - profile.z_lift_mm).abs() < 1e-4,
            "expected the profile's own {} mm of lift, got {}",
            profile.z_lift_mm,
            report.lifted_mm
        );
        // The slab's own eight corners, before the columns appended after them.
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
        let report = stand_under(&mut mesh, &SupportProfile::medium(), 0.05)
            .expect("a slab slices and supports");

        assert!(
            report.lifted_mm.abs() < 1e-6,
            "a slab 10 mm up has room already, got a lift of {}",
            report.lifted_mm
        );
    }
}
