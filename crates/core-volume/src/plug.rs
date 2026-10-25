use core_csg::Solid;
use core_geometry::{Aabb, Bvh, Mesh, Quat, Ray, Scalar, Transform, Vec3, cuboid, transform_mesh};
use rayon::prelude::*;

use crate::drain::{DrainHole, HoleKind, MOUTH_LIFT_MM, Section, body_of};
use crate::drilled::meets;
use crate::error::VolumeError;
use crate::hollow::HollowMode;
use crate::mend::readable;

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
        let Ok(whole) = readable(model) else {
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
        let body = Body::checked(hole, wall_mm, 0.0, index).ok()??;
        body.mesh(hole).aabb()
    })
}

/// The plugs cut for a model's holes, one a hole in their order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plugs {
    /// One plug a hole, in the model's own space; empty for a hole in the supports.
    pub meshes: Vec<Mesh>,
    /// How many of them had their outer face sampled rather than cut from the model, which
    /// the engine could not read even patched: they follow its surface only at the samples.
    pub sampled: usize,
    /// How many holes the gap closed at the mouth, each left an empty mesh.
    pub closed: usize,
}

/// How long a plug is for a model shelled by `mode` to `thickness_mm`: the wall, where the
/// cavity is inside it; `None`, the hole's own depth, for a mould, which has no wall to fill.
pub fn plug_wall_mm(mode: HollowMode, thickness_mm: Scalar) -> Option<Scalar> {
    (mode == HollowMode::Internal).then_some(thickness_mm)
}

/// A plug for each of `holes` as they are cut, after [`crate::pierce`], in the model's own
/// space: what the hole took out of the wall, `gap_mm` narrower all round (ADR 0321).
///
/// Its outer face is the very surface the hole took, its inner end flat, `wall_mm` in, or at
/// the hole's depth without one or for a walled hole. `source` is the model as last read.
pub fn plugs(
    source: &mut Option<PlugSource>,
    model: &Mesh,
    bvh: &Bvh,
    holes: &[DrainHole],
    wall_mm: Option<Scalar>,
    gap_mm: Scalar,
) -> Result<Plugs, VolumeError> {
    let bodies: Vec<Option<Body>> = holes
        .iter()
        .enumerate()
        .map(|(index, hole)| Body::checked(hole, wall_mm, gap_mm, index))
        .collect::<Result<_, VolumeError>>()?;
    let closed = bodies
        .iter()
        .zip(holes)
        .filter(|(body, hole)| body.is_none() && hole.kind != HoleKind::Support)
        .count();
    if bodies.iter().all(Option::is_none) {
        return Ok(Plugs {
            meshes: vec![Mesh::default(); bodies.len()],
            sampled: 0,
            closed,
        });
    }
    if !source
        .as_ref()
        .is_some_and(|read| read.covers(holes, wall_mm))
    {
        *source = Some(PlugSource::read(model, holes, wall_mm));
    }
    let solid = source.as_ref().and_then(PlugSource::readable);
    let cut = cut_each(solid, model, bvh, holes, &bodies);
    let sampled = cut.iter().filter(|(_, sampled)| *sampled).count();
    let meshes = cut.into_iter().map(|(mesh, _)| mesh).collect();
    if source.as_ref().is_some_and(PlugSource::is_whole) {
        *source = None;
    }
    Ok(Plugs {
        meshes,
        sampled,
        closed,
    })
}

/// The plug of each of `bodies`, cut from `solid` or, where there is none or the engine
/// fails, sampled off `model`, marked `true`; a hole the gap closes leaves an empty mesh.
fn cut_each(
    solid: Option<&Solid>,
    model: &Mesh,
    bvh: &Bvh,
    holes: &[DrainHole],
    bodies: &[Option<Body>],
) -> Vec<(Mesh, bool)> {
    bodies
        .par_iter()
        .enumerate()
        .map(|(index, body)| {
            let (hole, Some(body)) = (&holes[index], body) else {
                return (Mesh::default(), false);
            };
            match solid.map(|solid| intersected(solid, &body.mesh(hole), &holes[..index])) {
                Some(Ok(mesh)) => (mesh, false),
                // The engine failing on a model it read is met as one it could not read.
                None | Some(Err(_)) => (sampled(model, bvh, hole, body), true),
            }
        })
        .collect()
}

/// `plugs`, cut for `holes` in a model's own space, carried onto the plate by `transform`,
/// stood on the end that goes in, and laid in a row past the right-hand side of `beside`;
/// an empty one is left out.
pub fn laid_beside(
    plugs: &[Mesh],
    holes: &[DrainHole],
    transform: Transform,
    beside: Aabb,
) -> Vec<Mesh> {
    let mut x = beside.maxs.x + PLUG_SPACING_MM;
    let mut laid = Vec::with_capacity(plugs.len());
    for (plug, hole) in plugs.iter().zip(holes).filter(|(plug, _)| !plug.is_empty()) {
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

/// What of `solid` lies in `body` and out of every hole of `earlier` that meets it, as a
/// hole drilled over others finds them: empty for a hole wholly inside an earlier one.
fn intersected(solid: &Solid, body: &Mesh, earlier: &[DrainHole]) -> Result<Mesh, VolumeError> {
    let Some(reach) = body.aabb() else {
        return Ok(Mesh::default());
    };
    let drilled = earlier
        .iter()
        .filter_map(body_of)
        .filter(|cut| cut.aabb().is_some_and(|bounds| meets(&bounds, &reach)))
        .map(|cut| Solid::new(&cut))
        .collect::<Result<Vec<_>, _>>()?;
    let mut piece = Solid::new(body)?;
    if !drilled.is_empty() {
        piece = piece.difference(&Solid::union_all(&drilled)?)?;
    }
    Ok(solid.intersection(&piece)?.mesh())
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
    /// The body `hole`, `index` among the holes asked for, leaves a plug, `None` where the
    /// gap closes its mouth or the hole is in the supports, or why it cannot be drilled.
    fn checked(
        hole: &DrainHole,
        wall_mm: Option<Scalar>,
        gap_mm: Scalar,
        index: usize,
    ) -> Result<Option<Self>, VolumeError> {
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
        if hole.kind == HoleKind::Support {
            return Ok(None);
        }
        let wall_mm = wall_mm.filter(|_| hole.kind == HoleKind::Drain);
        let body = Self::of(hole, axis, wall_mm.unwrap_or(hole.depth_mm), gap_mm);
        Ok((body.outer_mm.min(body.lifted_mm) > THINNEST_MM).then_some(body))
    }

    fn of(hole: &DrainHole, axis: Vec3, length_mm: Scalar, gap_mm: Scalar) -> Self {
        let section = Section::of(hole.shape, hole.turn_deg);
        // A polygon's sides move in by the gap, so its corners move further than that.
        let inset = gap_mm / (std::f32::consts::PI / section.sides as Scalar).cos();
        let lift_mm = hole.lift_mm.max(MOUTH_LIFT_MM);
        let narrowing = hole.diameter_mm / 2.0 * (hole.taper - 1.0) / (hole.depth_mm + lift_mm);
        let radius_at =
            |along: Scalar| hole.diameter_mm / 2.0 + narrowing * (along + lift_mm) - inset;
        // A cone the gap closes before its inner end is cut off where its radius is THINNEST_MM.
        let length_mm = if radius_at(length_mm) < THINNEST_MM {
            (THINNEST_MM - radius_at(0.0)) / narrowing
        } else {
            length_mm
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
mod tests;
