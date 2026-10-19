use core_geometry::{Aabb, Bvh, Mesh, Quat, Ray, Scalar, Transform, Vec3, transform_mesh};

use crate::drain::{DrainHole, MOUTH_LIFT_MM, Section};
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

/// How long a plug is for a model shelled by `mode` to `thickness_mm`: the wall, where the
/// cavity is inside it; `None`, the hole's own depth, for a mould, which has no wall to fill.
pub fn plug_wall_mm(mode: HollowMode, thickness_mm: Scalar) -> Option<Scalar> {
    (mode == HollowMode::Internal).then_some(thickness_mm)
}

/// A plug for each of `holes` as they are cut, after [`crate::pierce`], in the model's own
/// space: what the hole took out of the wall, `gap_mm` narrower all round (ADR 0313).
///
/// Its outer face is `model`'s surface over the mouth, so it closes flush, and its inner
/// end is flat, `wall_mm` in, or at the hole's depth without one. A taper carries over.
pub fn plugs(
    model: &Mesh,
    bvh: &Bvh,
    holes: &[DrainHole],
    wall_mm: Option<Scalar>,
    gap_mm: Scalar,
) -> Result<Vec<Mesh>, VolumeError> {
    holes
        .iter()
        .enumerate()
        .map(|(index, hole)| plug(model, bvh, hole, wall_mm, gap_mm, index))
        .collect()
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

fn plug(
    model: &Mesh,
    bvh: &Bvh,
    hole: &DrainHole,
    wall_mm: Option<Scalar>,
    gap_mm: Scalar,
    index: usize,
) -> Result<Mesh, VolumeError> {
    let axis = hole
        .axis
        .try_normalize()
        .ok_or(VolumeError::BadDrain { index })?;
    let radius = hole.diameter_mm / 2.0;
    let sound = radius > 0.0 && hole.depth_mm > 0.0 && hole.taper > 0.0 && hole.taper.is_finite();
    if !sound || !gap_mm.is_finite() {
        return Err(VolumeError::BadDrain { index });
    }
    let length_mm = wall_mm.unwrap_or(hole.depth_mm);

    let section = Section::of(hole.shape, hole.turn_deg);
    // A polygon's sides move in by the gap, so its corners move further than that.
    let inset = gap_mm / (std::f32::consts::PI / section.sides as Scalar).cos();
    let lift = hole.lift_mm.max(MOUTH_LIFT_MM);
    let radius_at = |along: Scalar| {
        let reach = (along + lift) / (hole.depth_mm + lift);
        radius * (1.0 + (hole.taper - 1.0) * reach) - inset
    };
    let (outer, inner) = (radius_at(0.0), radius_at(length_mm));
    if outer <= 0.0 || inner <= 0.0 {
        return Err(VolumeError::TightPlug { index, gap_mm });
    }

    let corners: Vec<Vec3> = section.corners(axis).collect();
    let deepest = length_mm - THINNEST_MM;
    let face = |offset: Vec3| {
        let point = hole.at + offset;
        point + axis * surface_depth(model, bvh, point, axis, length_mm).min(deepest)
    };
    Ok(closed(
        &corners,
        hole.at + axis * length_mm,
        inner,
        outer,
        face,
    ))
}

/// How far along `axis` from `point` the model's surface is, read from inside the wall
/// outward the way [`crate::lift_for`] reads a rim: the first face the ray leaves the
/// material through. Nothing found reads as the mouth's own plane.
fn surface_depth(model: &Mesh, bvh: &Bvh, point: Vec3, axis: Vec3, length_mm: Scalar) -> Scalar {
    let start = length_mm - THINNEST_MM;
    let ray = Ray::new(point + axis * start, -axis);
    bvh.raycast(model, &ray)
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

    fn the_plug(hole: DrainHole, wall_mm: Option<Scalar>, gap_mm: Scalar) -> Mesh {
        let (mesh, bvh) = slab();
        let mut made = plugs(&mesh, &bvh, &[hole], wall_mm, gap_mm).expect("the hole is sound");
        made.pop().expect("one plug a hole")
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
            plugs(&mesh, &bvh, &[hole_in_the_top()], Some(2.0), 2.5),
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
            plugs(&mesh, &bvh, &[nowhere], None, 0.1),
            Err(VolumeError::BadDrain { index: 0 })
        );
    }
}
