use std::ops::Range;
use std::sync::Arc;

use core_geometry::{Bvh, Mesh, Scalar, Transform, Vec3, lift_over_plate, transform_mesh};
use core_slicer::{PlaneSliceEngine, SliceEngine, SliceSettings, Sliced};
use core_supports::{
    Blocked, Need, Placed, Profiles, SupportPoint, WallSide, columns, generate_supports, grow,
    low_points, mesh_trees,
};
use core_volume::Drilled;
use printer_profiles::{AUTO_SIZES, AutoProfile, Catalogue, ProfileError, SupportProfile};

use crate::error::EngineError;

/// What a run builds supports to: one profile, or the shapes an automatic profile sizes
/// each support into by what it holds (ADR 0325).
#[derive(Debug, Clone, PartialEq)]
pub enum SupportShapes {
    One(Box<SupportProfile>),
    Sized {
        auto: Box<AutoProfile>,
        /// The shapes it names, lightest first, then its steadying one.
        shapes: Box<[SupportProfile; AUTO_SIZES + 1]>,
    },
}

/// What a run stands under a model: the shapes it builds to, and whether it holds up a
/// hollowed model's cavity as well as its outside, as the window's two runs do (ADR 0332).
#[derive(Debug, Clone, PartialEq)]
pub struct SupportPlan {
    pub shapes: SupportShapes,
    pub cavity: bool,
}

impl SupportPlan {
    /// Supports built to `profile` alone, outside only.
    pub fn of(profile: SupportProfile) -> Self {
        Self {
            shapes: SupportShapes::One(Box::new(profile)),
            cavity: false,
        }
    }

    /// Supports sized by `auto` into the shapes `catalogue` has for it, outside only.
    pub fn sized(auto: AutoProfile, catalogue: &Catalogue) -> Result<Self, ProfileError> {
        let shapes = auto.shapes_in(catalogue)?;
        Ok(Self {
            shapes: SupportShapes::Sized {
                auto: Box::new(auto),
                shapes: Box::new(shapes),
            },
            cavity: false,
        })
    }

    /// The profile placement runs with: where supports go, how close, how far a model is
    /// lifted and what raft it stands on.
    pub fn placing(&self) -> SupportProfile {
        match &self.shapes {
            SupportShapes::One(profile) => profile.as_ref().clone(),
            SupportShapes::Sized { auto, shapes } => auto.placing_with(&shapes[0]),
        }
    }

    /// The profiles supports are built to, by group.
    pub fn table(&self) -> Vec<SupportProfile> {
        match &self.shapes {
            SupportShapes::One(profile) => vec![profile.as_ref().clone()],
            SupportShapes::Sized { shapes, .. } => shapes.to_vec(),
        }
    }

    /// Every profile of [`Self::table`], to set what all of them share, a raft or a lift.
    pub fn profiles_mut(&mut self) -> &mut [SupportProfile] {
        match &mut self.shapes {
            SupportShapes::One(profile) => std::slice::from_mut(profile.as_mut()),
            SupportShapes::Sized { shapes, .. } => shapes.as_mut_slice(),
        }
    }

    /// The automatic profile supports are sized by, if any.
    pub fn sizing(&self) -> Option<&AutoProfile> {
        match &self.shapes {
            SupportShapes::One(_) => None,
            SupportShapes::Sized { auto, .. } => Some(auto),
        }
    }

    /// The group of [`Self::table`] a support holding `need` is built to.
    pub fn group_of(&self, need: &Need) -> u16 {
        self.sizing()
            .map_or(0, |auto| u16::try_from(need.size_in(auto)).unwrap_or(0))
    }
}

/// What automatic placement put under one model.
#[derive(Debug, Clone, PartialEq)]
pub struct Stood {
    /// How far the model was stood off the plate to make room, millimetres; zero for one
    /// that already stood clear of it.
    pub lifted_mm: Scalar,
    /// Where each contact touches the model, plate coordinates, millimetres.
    pub contacts: Vec<Vec3>,
    /// The way the surface under each contact faces, unit, or zero where none was found.
    pub facing: Vec<Vec3>,
    /// How many contacts each profile of the plan's table was given, by group.
    pub sized: Vec<usize>,
    /// Contacts a column was stood under.
    pub standing: usize,
    pub trees: usize,
    /// The supports themselves, in plate coordinates.
    pub supports: Mesh,
}

/// A model to stand supports under, in plate coordinates: its mesh, which of its faces
/// bound its cavity once hollowed, and the bodies its holes cut out of it, which ride along
/// as it is lifted.
pub struct ToStand<'a> {
    pub mesh: &'a mut Mesh,
    pub cavity: Option<Range<usize>>,
    pub cuts: &'a mut Option<Mesh>,
    /// Whether the cuts are taken out exactly, as the window takes them out, or left to the
    /// winding rule: a search over poses cannot afford the engine on every one.
    pub drill_exactly: bool,
}

/// Stands supports under the model, lifted first by the plan's lift and raft, against what
/// it prints as, and answers with them: `docs/design/supports.md`.
pub fn stand_under(
    model: ToStand<'_>,
    plan: &SupportPlan,
    layer_height_mm: Scalar,
) -> Result<Stood, EngineError> {
    let lift_mm = plan.placing().lift_mm();
    let (lifted_mm, surface, inner) = touched(model, lift_mm)?;
    let bvh = Arc::new(Bvh::build(&surface));
    let needs = needs_under(&surface, &bvh, inner, plan, layer_height_mm)?;
    grown(lifted_mm, &surface, &bvh, &needs, plan)
}

/// The same for a model left standing on the plate: supports go only under what hangs
/// over it, and the plate holds the rest. A model that needs none gets none, and one that
/// needs some stands on the raft's top, as the window's raft stands it.
pub fn stand_seated(
    model: ToStand<'_>,
    plan: &SupportPlan,
    layer_height_mm: Scalar,
) -> Result<Stood, EngineError> {
    let raft = plan.placing().raft;
    let ToStand {
        mesh,
        cavity,
        cuts,
        drill_exactly,
    } = model;
    let seated = ToStand {
        mesh: &mut *mesh,
        cavity,
        cuts: &mut *cuts,
        drill_exactly,
    };
    let (mut lifted_mm, mut surface, inner) = touched(seated, 0.0)?;
    let mut bvh = Arc::new(Bvh::build(&surface));
    let mut needs = needs_under(&surface, &bvh, inner, plan, layer_height_mm)?;
    if raft.enabled && !needs.is_empty() {
        let up = Vec3::Z * raft.thickness_mm;
        let moved = Transform::from_translation(up);
        *mesh = transform_mesh(mesh, moved);
        if let Some(cuts) = cuts.as_mut() {
            *cuts = transform_mesh(cuts, moved);
        }
        surface = transform_mesh(&surface, moved);
        bvh = Arc::new(Bvh::build(&surface));
        for need in &mut needs {
            need.contact += up;
        }
        lifted_mm += up.z;
    }
    grown(lifted_mm, &surface, &bvh, &needs, plan)
}

/// The model lifted `lift_mm` clear of the plate with its cuts, then how far it moved, the
/// surface it prints as and which faces of that lie inside it.
fn touched(
    model: ToStand<'_>,
    lift_mm: Scalar,
) -> Result<(Scalar, Mesh, Option<Range<usize>>), EngineError> {
    let lifted_mm = lift(model.mesh, model.cuts, lift_mm)?;
    let cuts = model.cuts.as_ref();
    let (surface, inner) = printed(model.mesh, model.cavity, cuts, model.drill_exactly);
    Ok((lifted_mm, surface, inner))
}

/// What `mesh` prints as: its cuts taken out exactly where asked, as the window takes them
/// out, else appended for the winding rule, as they are where the engine refuses them
/// (ADR 0322); and its faces on the cavity and the lattice in it.
fn printed(
    mesh: &Mesh,
    cavity: Option<Range<usize>>,
    cuts: Option<&Mesh>,
    exactly: bool,
) -> (Mesh, Option<Range<usize>>) {
    let inside = |faces: usize, cavity: Range<usize>| cavity.start..faces.max(cavity.end);
    let Some(cuts) = cuts else {
        return (
            mesh.clone(),
            cavity.map(|cavity| inside(mesh.faces.len(), cavity)),
        );
    };
    if let Some(Ok(drilled)) = exactly.then(|| Drilled::of(mesh, cavity.clone(), cuts)) {
        let surface = Arc::unwrap_or_clone(drilled.mesh);
        let inner = drilled
            .cavity
            .map(|cavity| inside(surface.faces.len(), cavity));
        return (surface, inner);
    }
    let mut surface = mesh.clone();
    append(&mut surface, cuts);
    (
        surface,
        cavity.map(|cavity| inside(mesh.faces.len(), cavity)),
    )
}

/// Columns stood on `surface` under each of `needs`, built to the group its size names,
/// and what they came to.
fn grown(
    lifted_mm: Scalar,
    surface: &Mesh,
    bvh: &Bvh,
    needs: &[Need],
    plan: &SupportPlan,
) -> Result<Stood, EngineError> {
    let points: Vec<SupportPoint> = needs
        .iter()
        .map(|need| SupportPoint {
            group: plan.group_of(need),
            ..SupportPoint::new(need.contact)
        })
        .collect();
    let contacts: Vec<Vec3> = points.iter().map(|point| point.contact).collect();
    let table = plan.table();
    let mut sized = vec![0; table.len()];
    for point in &points {
        if let Some(count) = sized.get_mut(usize::from(point.group)) {
            *count += 1;
        }
    }

    let placed = Placed::new(surface, bvh, Transform::default());
    let profiles = Profiles::new(&table).ok_or(EngineError::EmptyPlate)?;
    let columns = columns(&points, &placed, profiles);
    let trees = grow(&columns, &placed, profiles);
    let facing = facing(surface, bvh, &contacts);
    Ok(Stood {
        lifted_mm,
        contacts,
        facing,
        sized,
        standing: trees
            .iter()
            .map(core_supports::SupportTree::tip_count)
            .sum(),
        trees: trees.len(),
        supports: mesh_trees(&trees, &placed, profiles),
    })
}

/// The way the surface of `drilled` under each contact faces, unit, or zero where the
/// contact finds none.
fn facing(drilled: &Mesh, bvh: &Bvh, contacts: &[Vec3]) -> Vec<Vec3> {
    contacts
        .iter()
        .map(|contact| {
            bvh.closest(*contact)
                .and_then(|hit| drilled.triangle(hit.face))
                .map_or(Vec3::ZERO, |triangle| {
                    triangle.normal_unnormalized().normalize_or_zero()
                })
        })
        .collect()
}

/// Where `surface`, standing where it is, needs holding up: what automatic placement would
/// touch from outside its wall, then inside its cavity where the plan asks, each contact
/// with what it holds.
fn needs_under(
    surface: &Mesh,
    bvh: &Arc<Bvh>,
    inner: Option<Range<usize>>,
    plan: &SupportPlan,
    layer_height_mm: Scalar,
) -> Result<Vec<Need>, EngineError> {
    let stack = PlaneSliceEngine
        .slice(
            surface,
            &SliceSettings {
                layer_height: layer_height_mm,
                ..SliceSettings::default()
            },
        )
        .map_err(EngineError::Supports)?;
    let run = Run {
        stack: &stack,
        layer_height_mm,
        placing: plan.placing(),
        sizing: plan.sizing(),
        lows: low_points(surface, Transform::default()),
    };
    let inner = inner.filter(|inner| !inner.is_empty());
    let side = |inside| {
        inner.clone().map(|inner| WallSide {
            bvh: Arc::clone(bvh),
            inner,
            into_model: core_geometry::Mat4::IDENTITY,
            inside,
        })
    };
    let mut needs = run.place(side(false));
    if plan.cavity && inner.is_some() {
        needs.extend(run.place(side(true)));
    }
    Ok(needs)
}

/// One cut stack and what placement on it is asked with.
struct Run<'a> {
    stack: &'a Sliced,
    layer_height_mm: Scalar,
    placing: SupportProfile,
    sizing: Option<&'a AutoProfile>,
    lows: Vec<Vec3>,
}

impl Run<'_> {
    /// What automatic placement touches, kept to `side` of the wall where there is one.
    fn place(&self, side: Option<WallSide>) -> Vec<Need> {
        let blocked = Blocked::keeping(None, side.clone());
        let mut needs = generate_supports(
            self.stack,
            self.layer_height_mm,
            &self.placing,
            self.sizing,
            &[],
            &self.lows,
            blocked.as_ref(),
            &mut |_| true,
        );
        if let Some(side) = side {
            needs.retain(|need| side.keeps(need.contact));
        }
        needs
    }
}

/// Stands `mesh` and what is cut out of it `lift_mm` clear of the plate, and answers how
/// far they moved. A model already standing at least that high is left where it is: the
/// lift makes room under a part, it does not place it.
fn lift(mesh: &mut Mesh, cuts: &mut Option<Mesh>, lift_mm: Scalar) -> Result<Scalar, EngineError> {
    let bounds = mesh.aabb().ok_or(EngineError::EmptyPlate)?;
    if bounds.mins.z >= lift_mm {
        return Ok(0.0);
    }
    let offset = lift_over_plate(&bounds, lift_mm);
    let moved = Transform::from_translation(offset);
    *mesh = transform_mesh(mesh, moved);
    if let Some(cuts) = cuts.as_mut() {
        *cuts = transform_mesh(cuts, moved);
    }
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

    fn medium() -> SupportPlan {
        SupportPlan::of(SupportProfile::shipped("medium").expect("the medium preset ships"))
    }

    /// `mesh` with no cavity, and `cuts` taken out of it.
    fn solid<'a>(mesh: &'a mut Mesh, cuts: &'a mut Option<Mesh>) -> ToStand<'a> {
        ToStand {
            mesh,
            cavity: None,
            cuts,
            drill_exactly: true,
        }
    }

    #[test]
    fn a_model_with_no_vertices_has_nothing_to_stand_under() {
        let error = stand_under(solid(&mut Mesh::default(), &mut None), &medium(), 0.05)
            .expect_err("an empty mesh cannot be supported");
        assert!(matches!(error, EngineError::EmptyPlate));
    }

    #[test]
    fn a_layer_of_no_height_cannot_be_cut_for_contacts() {
        let mut slab = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 10.0),
                Vec3::new(10.0, 0.0, 10.0),
                Vec3::new(0.0, 10.0, 10.0),
                Vec3::new(0.0, 0.0, 12.0),
            ],
            vec![[0, 2, 1], [0, 1, 3], [1, 2, 3], [2, 0, 3]],
        );
        let error = stand_under(solid(&mut slab, &mut None), &medium(), 0.0)
            .expect_err("no layer height, no stack");
        assert!(matches!(error, EngineError::Supports(_)));
    }

    /// Axis-aligned box from `low` to `high`, wound outward.
    fn cuboid(mesh: &mut Mesh, low: Vec3, high: Vec3) {
        let corner = |at: usize| {
            Vec3::new(
                if at & 1 == 0 { low.x } else { high.x },
                if at & 2 == 0 { low.y } else { high.y },
                if at & 4 == 0 { low.z } else { high.z },
            )
        };
        let box_mesh = Mesh::new(
            (0..8).map(corner).collect(),
            vec![
                [0, 2, 1],
                [1, 2, 3],
                [4, 5, 6],
                [5, 7, 6],
                [0, 1, 4],
                [1, 5, 4],
                [2, 6, 3],
                [3, 6, 7],
                [0, 4, 2],
                [2, 4, 6],
                [1, 3, 5],
                [3, 7, 5],
            ],
        );
        append(mesh, &box_mesh);
    }

    /// A 10 mm slab on a 2 mm stem standing on the plate: its underside hangs.
    fn mushroom() -> Mesh {
        let mut mesh = Mesh::default();
        cuboid(
            &mut mesh,
            Vec3::new(4.0, 4.0, 0.0),
            Vec3::new(6.0, 6.0, 11.0),
        );
        cuboid(
            &mut mesh,
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(10.0, 10.0, 12.0),
        );
        mesh
    }

    #[test]
    fn a_seated_model_with_supports_stands_on_its_rafts_top() {
        let mut profile = SupportProfile::shipped("medium").expect("the medium preset ships");
        profile.raft.enabled = true;
        let mut mesh = mushroom();
        let stood = stand_seated(
            solid(&mut mesh, &mut None),
            &SupportPlan::of(profile.clone()),
            0.05,
        )
        .expect("a mushroom on the plate can be supported");
        let low_mm = mesh.aabb().expect("the mushroom has bounds").mins.z;

        assert!(!stood.contacts.is_empty(), "the slab's underside hangs");
        assert!(
            (low_mm - profile.raft.thickness_mm).abs() < 1e-4,
            "the raft is not printed into the stem, got {low_mm}"
        );
        assert!((stood.lifted_mm - profile.raft.thickness_mm).abs() < 1e-4);
    }

    #[test]
    fn a_seated_model_needing_no_supports_stays_on_the_plate_over_a_raft() {
        let mut profile = SupportProfile::shipped("medium").expect("the medium preset ships");
        profile.raft.enabled = true;
        let mut mesh = Mesh::default();
        cuboid(&mut mesh, Vec3::ZERO, Vec3::splat(10.0));
        let stood = stand_seated(
            solid(&mut mesh, &mut None),
            &SupportPlan::of(profile.clone()),
            0.05,
        )
        .expect("a cube on the plate can be supported");

        assert!(stood.contacts.is_empty(), "nothing on a cube hangs");
        assert!(stood.lifted_mm.abs() < 1e-6, "no raft is built, so no lift");
    }
}
