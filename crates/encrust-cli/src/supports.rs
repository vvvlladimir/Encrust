//! Supports from the command line, the window's sequence without hand placement: find contacts,
//! columns, trunks, mesh, merge, and Merge into one by points. See `docs/design/supports.md`.

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use anyhow::{Context, Result};
use core_engine::SupportPlan;
use core_geometry::{Mesh, Scalar, Transform, Vec3, transform_mesh};
use core_supports::Unstood;
use core_volume::Drilled;

use crate::hollowing::{HollowReport, point};

/// The points one `--merge-supports` names supports by, plate mm: two or more.
#[derive(Debug, Clone, PartialEq)]
pub struct MergePoints(pub Vec<Vec3>);

/// `X,Y,Z;X,Y,Z…` as the points of one merge.
pub fn merge_points(text: &str) -> Result<MergePoints, String> {
    let points = text.split(';').map(point).collect::<Result<Vec<_>, _>>()?;
    if points.len() < 2 {
        return Err(format!("a merge names two supports or more, got {text:?}"));
    }
    Ok(MergePoints(points))
}

/// The supports standing under one model, plate coordinates: the trees, and the raft apart,
/// which is all of them a hole cuts (ADR 0441).
#[derive(Debug, Clone, Default)]
pub struct Standing {
    pub trees: Mesh,
    pub raft: Option<Mesh>,
}

impl Standing {
    /// The trees, then the raft.
    pub fn meshes(&self) -> impl Iterator<Item = &Mesh> {
        std::iter::once(&self.trees).chain(&self.raft)
    }

    /// Moves every mesh by `transform`.
    pub fn moved(&mut self, transform: Transform) {
        self.trees = transform_mesh(&self.trees, transform);
        if let Some(raft) = &mut self.raft {
            *raft = transform_mesh(raft, transform);
        }
    }

    /// Takes in `other`, stood beside these under the same model.
    pub fn append(&mut self, other: &Self) {
        self.trees.append(&other.trees);
        if let Some(more) = &other.raft {
            self.raft.get_or_insert_default().append(more);
        }
    }

    /// The trees and the raft as a run's model carries them.
    pub fn parts(&self) -> (Vec<Arc<Mesh>>, Option<Arc<Mesh>>) {
        let trees = (!self.trees.faces.is_empty()).then(|| Arc::new(self.trees.clone()));
        (trees.into_iter().collect(), self.raft.clone().map(Arc::new))
    }
}

/// What automatic placement put under one model.
pub struct SupportReport {
    /// How far the model was stood off the plate to make room, millimetres; zero for one
    /// that already stood clear of it.
    pub lifted_mm: Scalar,
    /// Where each contact touches the model, plate coordinates, millimetres.
    pub contacts: Vec<Vec3>,
    pub standing: usize,
    /// The contacts the run found but could not stand a column under, plate millimetres,
    /// each with why.
    pub unheld: Vec<(Vec3, Unstood)>,
    pub trees: usize,
    pub faces: usize,
}

impl SupportReport {
    /// How many contacts the run found but could not stand a column under.
    pub fn unsupported(&self) -> usize {
        self.unheld.len()
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
        for why in [Unstood::HeadBlocked, Unstood::FacesUp, Unstood::NoGround] {
            let count = self.unheld.iter().filter(|(_, lost)| *lost == why).count();
            if count > 0 {
                writeln!(f, "    {:<13}{count}", why_named(why).replace('-', " "))?;
            }
        }
        writeln!(f)
    }
}

/// How the report names why a contact has no support, in the terminal and in `--json`.
pub fn why_named(why: Unstood) -> &'static str {
    match why {
        Unstood::HeadBlocked => "head-blocked",
        Unstood::FacesUp => "faces-up",
        Unstood::NoGround => "no-ground",
    }
}

/// Which faces of a model bound its cavity, and how its cuts are taken out of it.
#[derive(Debug, Clone, Default)]
pub struct Printed {
    pub cavity: Option<Range<usize>>,
    /// Whether the faces before the cavity are the model byte for byte (ADR 0412).
    pub on_its_model: bool,
}

impl Printed {
    /// As `hollow` left the model, or as it came without one.
    pub fn of(cavity: Option<Range<usize>>, hollow: Option<&HollowReport>) -> Self {
        Self {
            cavity,
            on_its_model: hollow.is_some_and(HollowReport::on_its_model),
        }
    }

    /// `mesh` as it prints with `cuts` taken out exactly, as the window takes them out; `None`
    /// without cuts or where the engine refuses them, left to the winding rule.
    pub fn drilled(&self, mesh: &Mesh, cuts: Option<&Mesh>) -> Option<Arc<Mesh>> {
        let cavity = self.cavity.clone();
        Drilled::of_printed(mesh, cavity, self.on_its_model, cuts?)
            .ok()
            .map(|drilled| drilled.mesh)
    }
}

/// Supports under `mesh`, plate coordinates, by `stand_under`, or `stand_seated` for a model
/// left on the plate, with what it found laid out for a terminal.
pub fn stand_under(
    (mesh, cavity, cuts): (&mut Mesh, Printed, &mut Option<Mesh>),
    plan: &SupportPlan,
    layer_height_mm: Scalar,
    seated: bool,
) -> Result<(SupportReport, Standing)> {
    let stand = if seated {
        core_engine::stand_seated
    } else {
        core_engine::stand_under
    };
    let model = core_engine::ToStand {
        mesh,
        cavity: cavity.cavity,
        on_its_model: cavity.on_its_model,
        cuts,
        drill_exactly: true,
    };
    let stood =
        stand(model, plan, layer_height_mm).context("cannot stand supports under the model")?;
    Ok(reported(stood))
}

/// A support under each of `contacts` alone, plate mm, by `core_engine::stand_at`.
pub fn stand_at(
    (mesh, cavity, cuts): (&mut Mesh, Printed, &mut Option<Mesh>),
    plan: &SupportPlan,
    contacts: &[Vec3],
) -> Result<(SupportReport, Standing)> {
    let model = core_engine::ToStand {
        mesh,
        cavity: cavity.cavity,
        on_its_model: cavity.on_its_model,
        cuts,
        drill_exactly: true,
    };
    let stood = core_engine::stand_at(model, plan, contacts)
        .context("cannot stand supports under the islands")?;
    Ok(reported(stood))
}

impl SupportReport {
    /// What `other`, stood beside these on the same model, adds to them.
    pub fn add(&mut self, other: Self) {
        self.contacts.extend(other.contacts);
        self.standing += other.standing;
        self.unheld.extend(other.unheld);
        self.trees += other.trees;
        self.faces += other.faces;
    }
}

fn reported(stood: core_engine::Stood) -> (SupportReport, Standing) {
    let unheld = stood
        .unheld
        .iter()
        .filter_map(|lost| Some((*stood.contacts.get(lost.point)?, lost.why)))
        .collect();
    let report = SupportReport {
        lifted_mm: stood.lifted_mm,
        contacts: stood.contacts,
        standing: stood.standing,
        unheld,
        trees: stood.trees,
        faces: stood.supports.faces.len() + stood.raft.as_ref().map_or(0, |raft| raft.faces.len()),
    };
    let standing = Standing {
        trees: stood.supports,
        raft: stood.raft,
    };
    (report, standing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use printer_profiles::SupportProfile;

    fn medium() -> SupportPlan {
        SupportPlan::of(SupportProfile::shipped("medium").expect("the medium preset ships"))
    }

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
        let (report, supports) = stand_under(
            (&mut mesh, Printed::default(), &mut None),
            &medium(),
            0.05,
            false,
        )
        .expect("a slab slices and supports");

        assert!(!report.contacts.is_empty(), "a hanging slab needs holding");
        assert!(report.standing > 0, "and there is empty plate under it");
        assert_eq!(
            supports
                .meshes()
                .map(|mesh| mesh.faces.len())
                .sum::<usize>(),
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

        let (report, _) = stand_under(
            (&mut mesh, Printed::default(), &mut None),
            &SupportPlan::of(profile.clone()),
            0.05,
            false,
        )
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
        let (report, _) = stand_under(
            (&mut mesh, Printed::default(), &mut None),
            &medium(),
            0.05,
            false,
        )
        .expect("a slab slices and supports");

        assert!(
            report.lifted_mm.abs() < 1e-6,
            "a slab 10 mm up has room already, got a lift of {}",
            report.lifted_mm
        );
    }
}
