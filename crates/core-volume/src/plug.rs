use core_csg::Solid;
use core_geometry::{
    Aabb, Bvh, FastMap, FastSet, Mesh, Quat, Ray, Scalar, Transform, Vec3, cuboid, fill_holes,
    orient_outward, remove_duplicate_faces, transform_mesh,
};
use rayon::prelude::*;

use crate::boolean::solid_of;
use crate::drain::{DrainHole, MOUTH_LIFT_MM, Section, body_of};
use crate::error::VolumeError;
use crate::hollow::HollowMode;

/// Rings the outer face of a plug is sampled on between its middle and its rim. Four on a
/// twenty-millimetre hole is a sample every two and a half millimetres, where a surface
/// curved enough to part from a straight chord by a layer is one no drain is put in.
const FACE_RINGS: usize = 4;

/// The least a plug is long wherever its outer face is read deeper than its inner end,
/// millimetres: half a layer of the finest machine, so it still closes.
const THINNEST_MM: Scalar = 0.05;

/// How much narrower than its hole a plug is drawn when nothing else is asked, millimetres a
/// side: a slip fit on a resin printed to size, past the roughness of its walls.
pub const PLUG_GAP_MM: Scalar = 0.15;

/// Space left between a model and the plugs laid beside it, and between one plug and the
/// next, millimetres.
pub const PLUG_SPACING_MM: Scalar = 3.0;

/// How far past the holes the model read for their plugs is kept, millimetres: room for a
/// hole moved or added beside them to be cut from the same piece.
const CROP_MARGIN_MM: Scalar = 5.0;

/// The piece of a model round its holes, read once as the solid their plugs are cut from:
/// a change of gap, or a hole moved nearby, cuts only the plugs again (ADR 0321).
#[derive(Debug)]
pub struct PlugSource {
    solid: Option<Solid>,
    /// The boxes, one a hole, the solid holds the model in; `None`, all of it, where the
    /// crop failed or would keep the whole.
    regions: Option<Vec<Aabb>>,
}

impl PlugSource {
    /// `model` as the engine reads it, mended first where it refuses it as it stands, and
    /// kept only round what `holes` cut through a wall of `wall_mm`. A model it refuses even
    /// mended holds nothing, and its plugs are sampled.
    fn read(model: &Mesh, holes: &[DrainHole], wall_mm: Option<Scalar>) -> Self {
        let Ok(whole) = solid_of(model).or_else(|_| solid_of(&patched(model))) else {
            return Self {
                solid: None,
                regions: None,
            };
        };
        let margin = Vec3::splat(CROP_MARGIN_MM);
        let regions: Vec<Aabb> = reaches(holes, wall_mm)
            .map(|reach| Aabb::new(reach.mins - margin, reach.maxs + margin))
            .collect();
        let crops = model
            .aabb()
            .is_some_and(|bounds| !regions.iter().any(|region| contains(region, &bounds)));
        match crops.then(|| cropped(&whole, &regions)).flatten() {
            Some(solid) => Self {
                solid: Some(solid),
                regions: Some(regions),
            },
            None => Self {
                solid: Some(whole),
                regions: None,
            },
        }
    }

    /// Whether every plug `holes` ask for, at any gap, lies in what was kept.
    fn covers(&self, holes: &[DrainHole], wall_mm: Option<Scalar>) -> bool {
        let Some(regions) = &self.regions else {
            return true;
        };
        reaches(holes, wall_mm).all(|reach| regions.iter().any(|region| contains(region, &reach)))
    }

    /// Whether it holds the whole model read, which is too big to keep for the next cut.
    fn is_whole(&self) -> bool {
        self.solid.is_some() && self.regions.is_none()
    }

    /// The solid plugs are cut from, where the model was read as one with an inside.
    fn readable(&self) -> Option<&Solid> {
        self.solid.as_ref().filter(|solid| !solid.is_empty())
    }
}

/// What of `whole` lies in any of `regions`, or `None` where the engine fails.
fn cropped(whole: &Solid, regions: &[Aabb]) -> Option<Solid> {
    let boxes = regions
        .iter()
        .map(|region| Solid::new(&cuboid(region.mins, region.maxs)))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    whole.intersection(&Solid::union_all(&boxes).ok()?).ok()
}

fn contains(outer: &Aabb, inner: &Aabb) -> bool {
    outer.mins.cmple(inner.mins).all() && outer.maxs.cmpge(inner.maxs).all()
}

/// The box round each body `holes` could leave a plug, at no gap and so at the widest.
fn reaches(holes: &[DrainHole], wall_mm: Option<Scalar>) -> impl Iterator<Item = Aabb> {
    holes.iter().enumerate().filter_map(move |(index, hole)| {
        let body = Body::checked(hole, wall_mm, 0.0, index).ok()?;
        body.mesh(hole).aabb()
    })
}

/// The plugs cut for a model's holes, one a hole in their order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plugs {
    /// One plug a hole, in the model's own space.
    pub meshes: Vec<Mesh>,
    /// How many of them had their outer face sampled rather than cut from the model, which
    /// the engine could not read even patched: they follow its surface only at the samples.
    pub sampled: usize,
}

/// How long a plug is for a model shelled by `mode` to `thickness_mm`: the wall, where the
/// cavity is inside it; `None`, the hole's own depth, for a mould, which has no wall to fill.
pub fn plug_wall_mm(mode: HollowMode, thickness_mm: Scalar) -> Option<Scalar> {
    (mode == HollowMode::Internal).then_some(thickness_mm)
}

/// A plug for each of `holes` as they are cut, after [`crate::pierce`], in the model's own
/// space: what the hole took out of the wall, `gap_mm` narrower all round (ADR 0321).
///
/// It is `source` intersected with the hole's body, so its outer face is the very surface
/// the hole took, and its inner end is flat, `wall_mm` in, or at the hole's depth without
/// one. A `source` that is `None` or not read round these holes is read again from `model`
/// once the holes are found sound, and left for the next call unless it is the whole model;
/// where it holds no solid, each plug's outer face is sampled off `model` instead.
pub fn plugs(
    source: &mut Option<PlugSource>,
    model: &Mesh,
    bvh: &Bvh,
    holes: &[DrainHole],
    wall_mm: Option<Scalar>,
    gap_mm: Scalar,
) -> Result<Plugs, VolumeError> {
    let bodies: Vec<(&DrainHole, Body)> = holes
        .iter()
        .enumerate()
        .map(|(index, hole)| Ok((hole, Body::checked(hole, wall_mm, gap_mm, index)?)))
        .collect::<Result<_, VolumeError>>()?;
    if bodies.is_empty() {
        return Ok(Plugs::default());
    }
    if !source
        .as_ref()
        .is_some_and(|read| read.covers(holes, wall_mm))
    {
        *source = Some(PlugSource::read(model, holes, wall_mm));
    }
    let solid = source.as_ref().and_then(PlugSource::readable);
    let meshes: Vec<Mesh> = bodies
        .par_iter()
        .enumerate()
        .map(|(index, (hole, body))| {
            solid
                .and_then(|solid| intersected(solid, &body.mesh(hole), &holes[..index]))
                .unwrap_or_else(|| sampled(model, bvh, hole, body))
        })
        .collect();
    let sampled = if solid.is_some() { 0 } else { meshes.len() };
    if source.as_ref().is_some_and(PlugSource::is_whole) {
        *source = None;
    }
    Ok(Plugs { meshes, sampled })
}

/// `plugs`, cut for `holes` in a model's own space, carried onto the plate by `transform`,
/// stood on the end that goes in, and laid in a row past the right-hand side of `beside`.
pub fn laid_beside(
    plugs: &[Mesh],
    holes: &[DrainHole],
    transform: Transform,
    beside: Aabb,
) -> Vec<Mesh> {
    let mut x = beside.maxs.x + PLUG_SPACING_MM;
    let mut laid = Vec::with_capacity(plugs.len());
    for (plug, hole) in plugs.iter().zip(holes) {
        let placed = transform_mesh(plug, transform);
        let stood = stand_on_end(&placed, transform.linear() * hole.axis);
        let Some(size) = stood.aabb().map(|bounds| bounds.extents()) else {
            continue;
        };
        let offset = Vec3::new(x + size.x / 2.0, beside.mins.y + size.y / 2.0, 0.0);
        laid.push(transform_mesh(&stood, Transform::from_translation(offset)));
        x += size.x + PLUG_SPACING_MM;
    }
    laid
}

/// `plug` turned so the end that goes into the model is down, standing on the plate with
/// the middle of its footprint over the origin. `axis` is the hole's, carried the same way.
fn stand_on_end(plug: &Mesh, axis: Vec3) -> Mesh {
    let turn = axis.try_normalize().map_or(Quat::IDENTITY, |axis| {
        Quat::from_rotation_arc(axis, Vec3::NEG_Z)
    });
    let turned: Vec<Vec3> = plug.vertices.iter().map(|vertex| turn * *vertex).collect();
    let Some(bounds) = Mesh::new(turned.clone(), Vec::new()).aabb() else {
        return plug.clone();
    };
    let middle = (bounds.mins + bounds.maxs) / 2.0;
    let offset = Vec3::new(-middle.x, -middle.y, -bounds.mins.z);
    Mesh::new(
        turned.into_iter().map(|vertex| vertex + offset).collect(),
        plug.faces.clone(),
    )
}

/// `model` mended until the engine can read it, which a plug needs only near its hole: torn
/// and folded faces patched, then every face on an edge still unpaired cut out and filled.
fn patched(model: &Mesh) -> Mesh {
    let mut mesh = model.clone();
    remove_duplicate_faces(&mut mesh);
    orient_outward(&mut mesh);
    fill_holes(&mut mesh);
    let unsound = unpaired_corners(&mesh);
    if !unsound.is_empty() {
        mesh.faces
            .retain(|face| !face.iter().any(|corner| unsound.contains(corner)));
        fill_holes(&mut mesh);
    }
    orient_outward(&mut mesh);
    mesh
}

/// The corners of every edge not walked once each way, as a closed surface walks it.
fn unpaired_corners(mesh: &Mesh) -> FastSet<u32> {
    let mut walks: FastMap<(u32, u32), (u32, u32)> = FastMap::default();
    for face in &mesh.faces {
        for side in 0..3 {
            let (from, to) = (face[side], face[(side + 1) % 3]);
            let walked = walks.entry((from.min(to), from.max(to))).or_default();
            if from < to {
                walked.0 += 1;
            } else {
                walked.1 += 1;
            }
        }
    }
    walks
        .into_iter()
        .filter(|(_, walked)| *walked != (1, 1))
        .flat_map(|((low, high), _)| [low, high])
        .collect()
}

/// What of `solid` lies in `body` and out of every hole of `earlier` that meets it, as a
/// hole drilled over others finds them; `None` where the engine fails or nothing is left.
fn intersected(solid: &Solid, body: &Mesh, earlier: &[DrainHole]) -> Option<Mesh> {
    let reach = body.aabb()?;
    let drilled = earlier
        .iter()
        .filter_map(body_of)
        .filter(|cut| cut.aabb().is_some_and(|bounds| meets(&bounds, &reach)))
        .map(|cut| Solid::new(&cut))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let mut piece = Solid::new(body).ok()?;
    if !drilled.is_empty() {
        piece = piece.difference(&Solid::union_all(&drilled).ok()?).ok()?;
    }
    let taken = solid.intersection(&piece).ok()?;
    Some(taken.mesh()).filter(|mesh| !mesh.faces.is_empty())
}

fn meets(one: &Aabb, other: &Aabb) -> bool {
    one.mins.cmple(other.maxs).all() && one.maxs.cmpge(other.mins).all()
}

/// The body a plug fills: the hole's frustum moved in by the gap, from the lifted mouth to
/// its flat inner end `length_mm` in, with its radii at the mouth, the inner end and the
/// lifted mouth.
struct Body {
    axis: Vec3,
    lift_mm: Scalar,
    length_mm: Scalar,
    corners: Vec<Vec3>,
    outer_mm: Scalar,
    inner_mm: Scalar,
    lifted_mm: Scalar,
}

impl Body {
    /// The body `hole`, `index` among the holes asked for, leaves a plug, or why it has none.
    fn checked(
        hole: &DrainHole,
        wall_mm: Option<Scalar>,
        gap_mm: Scalar,
        index: usize,
    ) -> Result<Self, VolumeError> {
        let axis = hole
            .axis
            .try_normalize()
            .ok_or(VolumeError::BadDrain { index })?;
        let sound = hole.diameter_mm > 0.0
            && hole.depth_mm > 0.0
            && hole.taper > 0.0
            && hole.taper.is_finite();
        if !sound || !gap_mm.is_finite() {
            return Err(VolumeError::BadDrain { index });
        }
        let body = Self::of(hole, axis, wall_mm.unwrap_or(hole.depth_mm), gap_mm);
        if body.outer_mm.min(body.inner_mm).min(body.lifted_mm) <= 0.0 {
            return Err(VolumeError::TightPlug { index, gap_mm });
        }
        Ok(body)
    }

    fn of(hole: &DrainHole, axis: Vec3, length_mm: Scalar, gap_mm: Scalar) -> Self {
        let section = Section::of(hole.shape, hole.turn_deg);
        // A polygon's sides move in by the gap, so its corners move further than that.
        let inset = gap_mm / (std::f32::consts::PI / section.sides as Scalar).cos();
        let lift_mm = hole.lift_mm.max(MOUTH_LIFT_MM);
        let radius_at = |along: Scalar| {
            let reach = (along + lift_mm) / (hole.depth_mm + lift_mm);
            hole.diameter_mm / 2.0 * (1.0 + (hole.taper - 1.0) * reach) - inset
        };
        Self {
            axis,
            lift_mm,
            length_mm,
            corners: section.corners(axis).collect(),
            outer_mm: radius_at(0.0),
            inner_mm: radius_at(length_mm),
            lifted_mm: radius_at(-lift_mm),
        }
    }

    /// The body as a closed frustum wound outward, from the lifted mouth to the inner end.
    fn mesh(&self, hole: &DrainHole) -> Mesh {
        let mouth = hole.at - self.axis * self.lift_mm;
        let end = hole.at + self.axis * self.length_mm;
        let sides = self.corners.len() as u32;
        let mut vertices: Vec<Vec3> = self
            .corners
            .iter()
            .map(|c| mouth + *c * self.lifted_mm)
            .collect();
        vertices.extend(
            self.corners
                .iter()
                .map(|corner| end + *corner * self.inner_mm),
        );
        vertices.extend([mouth, end]);
        let (middle_mouth, middle_end) = (2 * sides, 2 * sides + 1);
        let mut faces = Vec::with_capacity(4 * sides as usize);
        for side in 0..sides {
            let next = (side + 1) % sides;
            // The mouth looks against the axis the corners turn about, as in `closed`.
            faces.push([middle_mouth, next, side]);
            faces.push([side, sides + next, sides + side]);
            faces.push([side, next, sides + next]);
            faces.push([middle_end, sides + side, sides + next]);
        }
        Mesh::new(vertices, faces)
    }
}

/// The body with its outer face sampled on rings over the mouth, each point dropped onto
/// `model`'s surface by a ray, for a model the engine cannot read as a solid.
fn sampled(model: &Mesh, bvh: &Bvh, hole: &DrainHole, body: &Body) -> Mesh {
    let deepest = body.length_mm - THINNEST_MM;
    let face = |offset: Vec3| {
        let point = hole.at + offset;
        let depth = surface_depth(model, bvh, point, body.axis, body.length_mm);
        point + body.axis * depth.min(deepest)
    };
    closed(
        &body.corners,
        hole.at + body.axis * body.length_mm,
        body.inner_mm,
        body.outer_mm,
        face,
    )
}

/// How far along `axis` from `point` the model's surface is, read from inside the wall
/// outward the way [`crate::lift_for`] reads a rim: the first face the ray leaves the
/// material through. Nothing found reads as the mouth's own plane.
fn surface_depth(model: &Mesh, bvh: &Bvh, point: Vec3, axis: Vec3, length_mm: Scalar) -> Scalar {
    let start = length_mm - THINNEST_MM;
    let ray = Ray::new(point + axis * start, -axis);
    bvh.raycast(&ray)
        .filter(|hit| {
            model
                .triangle(hit.face)
                .is_some_and(|face| face.normal_unnormalized().dot(axis) < 0.0)
        })
        .map_or(0.0, |hit| start - hit.t)
}

/// The plug as one closed surface wound outward: its outer face on rings over the mouth,
/// each point put where `face` reads it, its sides down to the flat inner end at `end`.
fn closed(
    corners: &[Vec3],
    end: Vec3,
    inner: Scalar,
    outer: Scalar,
    face: impl Fn(Vec3) -> Vec3,
) -> Mesh {
    let sides = corners.len() as u32;
    let mut vertices = vec![face(Vec3::ZERO)];
    for ring in 1..=FACE_RINGS {
        let radius = outer * ring as Scalar / FACE_RINGS as Scalar;
        vertices.extend(corners.iter().map(|corner| face(*corner * radius)));
    }
    let rim = 1 + (FACE_RINGS as u32 - 1) * sides;
    let floor = vertices.len() as u32;
    vertices.extend(corners.iter().map(|corner| end + *corner * inner));
    let middle = vertices.len() as u32;
    vertices.push(end);

    let at = |ring: u32, side: u32| 1 + (ring - 1) * sides + side % sides;
    let mut faces = Vec::new();
    for side in 0..sides {
        let next = side + 1;
        // The outer face looks out of the model, against the axis the corners turn about.
        faces.push([0, at(1, next), at(1, side)]);
        for ring in 1..FACE_RINGS as u32 {
            faces.push([at(ring, side), at(ring, next), at(ring + 1, next)]);
            faces.push([at(ring, side), at(ring + 1, next), at(ring + 1, side)]);
        }
        let (top, top_next) = (rim + side, rim + next % sides);
        let (low, low_next) = (floor + side, floor + next % sides);
        faces.push([top, low_next, low]);
        faces.push([top, top_next, low_next]);
        faces.push([middle, low, low_next]);
    }
    Mesh::new(vertices, faces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drain::HoleShape;
    use core_geometry::{cuboid, signed_volume};

    /// A slab 20 mm square and 10 mm thick, its top face at z = 10.
    fn slab() -> (Mesh, Bvh) {
        let mesh = cuboid(Vec3::ZERO, Vec3::new(20.0, 20.0, 10.0));
        let bvh = Bvh::build(&mesh);
        (mesh, bvh)
    }

    fn hole_in_the_top() -> DrainHole {
        DrainHole {
            at: Vec3::new(10.0, 10.0, 10.0),
            axis: Vec3::NEG_Z,
            diameter_mm: 4.0,
            depth_mm: 6.0,
            taper: 1.0,
            lift_mm: 0.0,
            shape: HoleShape::Round,
            turn_deg: 0.0,
        }
    }

    /// `plugs` from `model` read afresh, as a front end asks for them the first time.
    fn cut_plugs(
        model: &Mesh,
        bvh: &Bvh,
        holes: &[DrainHole],
        wall_mm: Option<Scalar>,
        gap_mm: Scalar,
    ) -> Result<Plugs, VolumeError> {
        plugs(&mut None, model, bvh, holes, wall_mm, gap_mm)
    }

    /// A bar 100 mm long, with a hole at each end of its top.
    fn bar_with_holes_far_apart() -> (Mesh, Bvh, DrainHole, DrainHole) {
        let mesh = cuboid(Vec3::ZERO, Vec3::new(100.0, 20.0, 10.0));
        let bvh = Bvh::build(&mesh);
        let near = DrainHole {
            at: Vec3::new(10.0, 10.0, 10.0),
            ..hole_in_the_top()
        };
        let far = DrainHole {
            at: Vec3::new(90.0, 10.0, 10.0),
            ..hole_in_the_top()
        };
        (mesh, bvh, near, far)
    }

    #[test]
    fn a_model_read_round_one_hole_covers_its_gap_and_its_neighbour_but_not_far_off() {
        let (mesh, _, near, far) = bar_with_holes_far_apart();
        let source = PlugSource::read(&mesh, &[near], Some(2.0));
        let beside = DrainHole {
            at: near.at + Vec3::new(3.0, 0.0, 0.0),
            ..near
        };
        assert!(
            source.covers(&[near], Some(2.0)),
            "the hole it was read for"
        );
        assert!(
            source.covers(&[beside], Some(2.0)),
            "a hole moved 3 mm, within the margin"
        );
        assert!(!source.covers(&[far], Some(2.0)), "a hole 80 mm away");
    }

    #[test]
    fn a_hole_past_what_was_read_is_cut_from_the_model_read_again() {
        let (mesh, bvh, near, far) = bar_with_holes_far_apart();
        let mut source = Some(PlugSource::read(&mesh, &[near], Some(2.0)));
        let made = plugs(&mut source, &mesh, &bvh, &[far], Some(2.0), 0.1).expect("sound");
        // Cut from the piece round the other hole, it would meet nothing and be sampled.
        assert_eq!(
            made.sampled, 0,
            "the far plug is cut from the model, not sampled"
        );
        assert!(
            source.is_some_and(|read| read.covers(&[far], Some(2.0))),
            "and the piece read round it is left for the next cut"
        );
    }

    #[test]
    fn a_gap_that_closes_a_widening_hole_at_its_lifted_mouth_leaves_no_plug() {
        let (mesh, bvh) = slab();
        let widening = DrainHole {
            taper: 2.0,
            ..hole_in_the_top()
        };
        // 2 mm of radius at the lifted mouth, still 2.0165 at the surface: 2.01 closes only
        // the first.
        assert_eq!(
            cut_plugs(&mesh, &bvh, &[widening], Some(2.0), 2.01),
            Err(VolumeError::TightPlug {
                index: 0,
                gap_mm: 2.01
            })
        );
    }

    #[test]
    fn holes_far_apart_keep_only_the_model_round_each() {
        let (mesh, bvh, near, far) = bar_with_holes_far_apart();
        let mut source = None;
        plugs(&mut source, &mesh, &bvh, &[near, far], Some(2.0), 0.1).expect("sound");
        let read = source.expect("a piece of the bar is kept");
        let middle = DrainHole {
            at: Vec3::new(50.0, 10.0, 10.0),
            ..near
        };
        assert!(
            read.covers(&[near, far], Some(2.0)),
            "both holes it was read for"
        );
        assert!(
            !read.covers(&[middle], Some(2.0)),
            "but not the 70 mm of bar between them"
        );
    }

    #[test]
    fn a_model_held_whole_is_not_kept_for_the_next_cut() {
        let (mesh, bvh) = slab();
        let everywhere = DrainHole {
            diameter_mm: 40.0,
            depth_mm: 12.0,
            ..hole_in_the_top()
        };
        let mut source = None;
        plugs(&mut source, &mesh, &bvh, &[everywhere], None, 0.1).expect("sound");
        assert!(
            source.is_none(),
            "a box round the whole slab crops nothing worth keeping"
        );
    }

    #[test]
    fn a_plug_drilled_over_an_earlier_hole_has_that_hole_through_it() {
        let (mesh, bvh) = slab();
        let narrow = DrainHole {
            diameter_mm: 3.0,
            ..hole_in_the_top()
        };
        let wide = DrainHole {
            diameter_mm: 10.0,
            ..hole_in_the_top()
        };
        let volume = |plug: &Mesh| signed_volume(plug);
        let made = cut_plugs(&mesh, &bvh, &[narrow, wide], Some(2.0), 0.1).expect("sound");
        let (alone, taken) = (
            the_plug(wide, Some(2.0), 0.1),
            the_plug(narrow, Some(2.0), 0.0),
        );
        // The wide plug less what the narrow hole took out of the same 2 mm of wall.
        let expected = volume(&alone) - volume(&taken);
        assert!(
            (volume(&made.meshes[1]) - expected).abs() < 1e-2,
            "the 10 mm plug has the 3 mm hole through it: {} against {expected}",
            volume(&made.meshes[1])
        );
        assert!(
            (volume(&made.meshes[0]) - volume(&the_plug(narrow, Some(2.0), 0.1))).abs() < 1e-3,
            "the 3 mm plug, drilled first, is whole and closes that hole"
        );

        let reversed = cut_plugs(&mesh, &bvh, &[wide, narrow], Some(2.0), 0.1).expect("sound");
        assert!(
            (volume(&reversed.meshes[0]) - volume(&alone)).abs() < 1e-3,
            "a hole drilled later leaves an earlier plug whole"
        );
    }

    #[test]
    fn an_unsound_hole_fails_before_the_model_is_read() {
        let (mesh, bvh) = slab();
        let nowhere = DrainHole {
            axis: Vec3::ZERO,
            ..hole_in_the_top()
        };
        let mut source = None;
        let made = plugs(&mut source, &mesh, &bvh, &[nowhere], None, 0.1);
        assert_eq!(made, Err(VolumeError::BadDrain { index: 0 }));
        assert!(
            source.is_none(),
            "nothing was read for a cut that cannot be made"
        );
    }

    fn the_plug(hole: DrainHole, wall_mm: Option<Scalar>, gap_mm: Scalar) -> Mesh {
        let (mesh, bvh) = slab();
        let mut made = cut_plugs(&mesh, &bvh, &[hole], wall_mm, gap_mm).expect("the hole is sound");
        assert_eq!(made.sampled, 0, "a closed slab is cut, not sampled");
        made.meshes.pop().expect("one plug a hole")
    }

    #[test]
    fn a_plug_through_a_wall_fills_the_wall_less_its_gap() {
        let plug = the_plug(hole_in_the_top(), Some(2.0), 0.1);
        let volume = signed_volume(&plug);
        // The hole is a 24-gon with its corners on 2 mm, so its sides stand 2 cos(pi / 24)
        // off the axis; the plug's stand 0.1 mm in from those, and a regular n-gon of
        // apothem a holds n a^2 tan(pi / n). Over the 2 mm wall.
        let sides: Scalar = 24.0;
        let half_angle = std::f32::consts::PI / sides;
        let apothem = 2.0 * half_angle.cos() - 0.1;
        let expected = sides * apothem * apothem * half_angle.tan() * 2.0;
        assert!(
            (volume - expected).abs() < 1e-3 * expected,
            "a plug 0.1 mm in from the hole's sides holds {expected} mm3, got {volume}"
        );
    }

    #[test]
    fn a_plug_drilled_over_a_point_is_the_point_it_took() {
        let pyramid = Mesh::new(
            vec![
                Vec3::new(-5.0, -5.0, 0.0),
                Vec3::new(5.0, -5.0, 0.0),
                Vec3::new(5.0, 5.0, 0.0),
                Vec3::new(-5.0, 5.0, 0.0),
                Vec3::new(0.0, 0.0, 10.0),
            ],
            vec![
                [0, 3, 2],
                [0, 2, 1],
                [0, 1, 4],
                [1, 2, 4],
                [2, 3, 4],
                [3, 0, 4],
            ],
        );
        let bvh = Bvh::build(&pyramid);
        let over_the_apex = DrainHole {
            at: Vec3::new(0.0, 0.0, 10.0),
            diameter_mm: 6.0,
            depth_mm: 3.0,
            ..hole_in_the_top()
        };
        let made =
            cut_plugs(&pyramid, &bvh, &[over_the_apex], None, 0.1).expect("the hole is sound");
        let volume = signed_volume(&made.meshes[0]);
        // 3 mm down a pyramid of base 10 and height 10 it is 3 mm square, all inside the
        // hole, so the plug is the tip itself: a third of 9 mm2 over 3 mm.
        assert!(
            (volume - 9.0).abs() < 1e-3,
            "the plug is the 9 mm3 tip the hole took, not a cylinder, got {volume}"
        );
    }

    #[test]
    fn a_torn_model_is_patched_for_the_engine_and_still_gives_the_point_it_took() {
        let pyramid = Mesh::new(
            vec![
                Vec3::new(-5.0, -5.0, 0.0),
                Vec3::new(5.0, -5.0, 0.0),
                Vec3::new(5.0, 5.0, 0.0),
                Vec3::new(-5.0, 5.0, 0.0),
                Vec3::new(0.0, 0.0, 10.0),
            ],
            // Half the base is missing, so the engine refuses the model as it stands.
            vec![[0, 3, 2], [0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]],
        );
        let bvh = Bvh::build(&pyramid);
        let over_the_apex = DrainHole {
            at: Vec3::new(0.0, 0.0, 10.0),
            diameter_mm: 6.0,
            depth_mm: 3.0,
            ..hole_in_the_top()
        };
        let made =
            cut_plugs(&pyramid, &bvh, &[over_the_apex], None, 0.1).expect("the hole is sound");
        let volume = signed_volume(&made.meshes[0]);
        // As in the closed pyramid: the 3 mm tip, a third of 9 mm2 over 3 mm.
        assert!(
            (volume - 9.0).abs() < 1e-3,
            "a tear far from the hole still leaves the 9 mm3 tip, not a cylinder, got {volume}"
        );
    }

    #[test]
    fn a_plug_no_patch_can_cut_is_sampled_and_counted() {
        // A lone square has no inside, so even closed over it leaves the hole nothing.
        let sheet = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 10.0),
                Vec3::new(20.0, 0.0, 10.0),
                Vec3::new(20.0, 20.0, 10.0),
                Vec3::new(0.0, 20.0, 10.0),
            ],
            vec![[0, 1, 2], [0, 2, 3]],
        );
        let bvh = Bvh::build(&sheet);
        let made = cut_plugs(&sheet, &bvh, &[hole_in_the_top()], Some(2.0), 0.1).expect("sound");
        assert_eq!(made.sampled, 1, "the one plug is sampled, and said to be");
        assert_eq!(made.meshes.len(), 1, "a hole still gets its plug");
    }

    #[test]
    fn a_torn_model_still_gets_a_plug_through_its_wall() {
        let (mut mesh, _) = slab();
        let underneath = mesh
            .faces
            .iter()
            .position(|face| face.iter().all(|at| mesh.vertices[*at as usize].z == 0.0))
            .expect("the slab has a floor");
        mesh.faces.remove(underneath);
        let bvh = Bvh::build(&mesh);
        let made = cut_plugs(&mesh, &bvh, &[hole_in_the_top()], Some(2.0), 0.1).expect("sound");
        let bounds = made.meshes[0].aabb().expect("a plug has vertices");
        assert!(
            (bounds.maxs.z - 10.0).abs() < 1e-4 && (bounds.mins.z - 8.0).abs() < 1e-4,
            "a torn floor still leaves a plug through the 2 mm wall: {bounds:?}"
        );
    }

    #[test]
    fn a_plug_is_flush_with_the_surface_it_closes() {
        let plug = the_plug(hole_in_the_top(), Some(2.0), 0.1);
        let bounds = plug.aabb().expect("a plug has vertices");
        assert!(
            (bounds.maxs.z - 10.0).abs() < 1e-4 && (bounds.mins.z - 8.0).abs() < 1e-4,
            "the plug runs from the slab's top down through the 2 mm wall, got {bounds:?}"
        );
    }

    #[test]
    fn a_square_hole_takes_a_square_plug() {
        let square = DrainHole {
            shape: HoleShape::Polygon { sides: 4 },
            ..hole_in_the_top()
        };
        let plug = the_plug(square, Some(2.0), 0.0);
        let volume = signed_volume(&plug);
        // Corners on a 2 mm radius make a square of side 2 root 2, 8 mm2, over 2 mm.
        assert!(
            (volume - 16.0).abs() < 1e-3,
            "a square plug with no gap fills the square hole, got {volume}"
        );
    }

    #[test]
    fn a_plug_stands_on_the_end_that_goes_in() {
        let plug = the_plug(hole_in_the_top(), Some(2.0), 0.1);
        let stood = stand_on_end(&plug, Vec3::NEG_Z);
        let bounds = stood.aabb().expect("a plug has vertices");
        assert!(bounds.mins.z.abs() < 1e-5, "it stands on the plate");
        assert!(
            (bounds.maxs.z - 2.0).abs() < 1e-4,
            "drilled downward, the plug already stands on its inner end, 2 mm tall: {bounds:?}"
        );
        assert!(
            (bounds.mins.x + bounds.maxs.x).abs() < 1e-4,
            "its footprint is centred on the origin"
        );
        assert!(signed_volume(&stood) > 0.0, "a turn keeps it wound outward");
    }

    #[test]
    fn a_tapered_plug_narrows_with_the_hole_as_it_is_cut_through_the_wall() {
        let shallow = DrainHole {
            depth_mm: 2.0,
            taper: 0.5,
            ..hole_in_the_top()
        };
        let cut = crate::pierce(&[shallow], 4.0, 0.2);
        let plug = the_plug(cut[0], Some(4.0), 0.1);
        let inner_x = plug
            .vertices
            .iter()
            .filter(|vertex| (vertex.z - 6.0).abs() < 1e-4)
            .map(|vertex| vertex.x - 10.0)
            .fold(Scalar::MIN, Scalar::max);
        // Pierced to 4 + 2 voxels from a mouth lifted MOUTH_LIFT_MM, the cone halves over
        // 4.45 mm, so 4 mm in a corner is 2 (1 - 0.5 * 4.05 / 4.45) out, less the gap's reach.
        let inset = 0.1 / (std::f32::consts::PI / 24.0).cos();
        let expected = 2.0 * (1.0 - 0.5 * 4.05 / 4.45) - inset;
        assert!(
            (inner_x - expected).abs() < 1e-3,
            "the plug's inner end is the hole there less its gap, {expected} mm, got {inner_x}"
        );
    }

    #[test]
    fn plugs_are_laid_in_a_row_on_the_plate_past_their_model() {
        let plug = the_plug(hole_in_the_top(), Some(2.0), 0.1);
        let beside = Aabb::new(Vec3::ZERO, Vec3::new(20.0, 20.0, 10.0));
        let laid = laid_beside(
            &[plug.clone(), plug],
            &[hole_in_the_top(); 2],
            Transform::default(),
            beside,
        );
        let bounds: Vec<Aabb> = laid
            .iter()
            .map(|plug| plug.aabb().expect("a plug has vertices"))
            .collect();
        assert!(
            (bounds[0].mins.x - (20.0 + PLUG_SPACING_MM)).abs() < 1e-4
                && (bounds[1].mins.x - (bounds[0].maxs.x + PLUG_SPACING_MM)).abs() < 1e-4,
            "a spacing past the model, then a spacing past the plug before: {bounds:?}"
        );
        assert!(
            bounds
                .iter()
                .all(|b| b.mins.z.abs() < 1e-5 && b.mins.y.abs() < 1e-4),
            "each stands on the plate, level with the model's front: {bounds:?}"
        );
    }

    #[test]
    fn a_gap_wider_than_the_hole_leaves_no_plug() {
        let (mesh, bvh) = slab();
        assert_eq!(
            cut_plugs(&mesh, &bvh, &[hole_in_the_top()], Some(2.0), 2.5),
            Err(VolumeError::TightPlug {
                index: 0,
                gap_mm: 2.5
            })
        );
    }

    #[test]
    fn a_hole_with_no_axis_has_no_plug() {
        let (mesh, bvh) = slab();
        let nowhere = DrainHole {
            axis: Vec3::ZERO,
            ..hole_in_the_top()
        };
        assert_eq!(
            cut_plugs(&mesh, &bvh, &[nowhere], None, 0.1),
            Err(VolumeError::BadDrain { index: 0 })
        );
    }
}
