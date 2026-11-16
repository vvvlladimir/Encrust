use core_geometry::{Aabb, BoundingVolume, Mesh, Scalar, Vec3, Winding, diagnose, signed_volume};
use rayon::prelude::*;

use super::STAND_OFF;

/// How far a corner is drawn toward its face's centroid before it is asked, so it asks of
/// the face and not of the edge it shares with another.
const INTO_THE_FACE: Scalar = 0.1;

/// The `pieces` that `bounding` marks, joined, less every face a closed bounding piece holds;
/// see `docs/decisions/0450-faces-buried-in-another-body-are-left-out-of-the-field.md`.
pub(super) fn unburied(pieces: &[Mesh], bounding: &[bool]) -> Mesh {
    let bounds: Vec<Option<Aabb>> = pieces
        .iter()
        .zip(bounding)
        .map(|(piece, bounds)| piece.aabb().filter(|_| *bounds))
        .collect();
    let met = meeting(&bounds);
    let closed: Vec<Option<Scalar>> = pieces
        .par_iter()
        .zip(&met)
        .map(|(piece, met)| {
            (!met.is_empty() && diagnose(piece).is_closed()).then(|| signed_volume(piece))
        })
        .collect();
    let burying: Vec<Option<Winding>> = pieces
        .par_iter()
        .zip(&closed)
        .map(|(piece, volume)| {
            volume
                .is_some_and(|volume| volume > 0.0)
                .then(|| Winding::build(piece))
        })
        .collect();

    let mut surface = Mesh::default();
    for (index, piece) in pieces.iter().enumerate() {
        let Some(own) = bounds[index] else {
            continue;
        };
        let others: Vec<Other<'_>> = met[index]
            .iter()
            .filter_map(|other| Some((bounds[*other]?, &pieces[*other], burying[*other].as_ref()?)))
            .collect();
        // A shell wound inward is the wall of a void the model already has: nothing buries it.
        let hollow = closed[index].is_some_and(|volume| volume < 0.0);
        if hollow || others.is_empty() {
            surface.append(piece);
            continue;
        }
        let stand_off = (own.maxs - own.mins).length() * STAND_OFF;
        let faces = piece
            .faces
            .par_iter()
            .enumerate()
            .filter(|(face, _)| !is_buried(piece, *face, &others, stand_off))
            .map(|(_, corners)| *corners)
            .collect();
        surface.append(&Mesh::new(piece.vertices.clone(), faces));
    }
    surface
}

/// For each of `bounds`, the others it meets, found by a sweep along x rather than by
/// testing every pair.
fn meeting(bounds: &[Option<Aabb>]) -> Vec<Vec<usize>> {
    let mut order: Vec<(usize, Aabb)> = bounds
        .iter()
        .enumerate()
        .filter_map(|(index, bounds)| Some((index, (*bounds)?)))
        .collect();
    order.sort_unstable_by(|one, other| one.1.mins.x.total_cmp(&other.1.mins.x));
    let mut met = vec![Vec::new(); bounds.len()];
    for (at, (one, own)) in order.iter().enumerate() {
        for (other, theirs) in order[at + 1..]
            .iter()
            .take_while(|(_, theirs)| theirs.mins.x <= own.maxs.x)
        {
            if own.intersects(theirs) {
                met[*one].push(*other);
                met[*other].push(*one);
            }
        }
    }
    met
}

/// A closed piece of the surface another's box meets: its box, its faces and its winding number.
type Other<'a> = (Aabb, &'a Mesh, &'a Winding);

/// Whether face `face` of `piece` has `others` just outside its middle and each corner:
/// standing off it, a face laid against another body's is read as buried in it.
fn is_buried(piece: &Mesh, face: usize, others: &[Other<'_>], stand_off: Scalar) -> bool {
    let Some(triangle) = piece.triangle(face) else {
        return false;
    };
    let Some(normal) = triangle.normal_unnormalized().try_normalize() else {
        return false;
    };
    let corners = [triangle.a, triangle.b, triangle.c];
    let middle = (corners[0] + corners[1] + corners[2]) / 3.0;
    let inside = |at: Vec3| {
        let at = at + normal * stand_off;
        let held: Scalar = others
            .iter()
            .filter(|(bounds, _, _)| bounds.contains_local_point(at))
            .map(|(_, mesh, winding)| winding.at(mesh, at))
            .sum();
        held > 0.5
    };
    inside(middle)
        && corners
            .iter()
            .all(|corner| inside(corner.lerp(middle, INTO_THE_FACE)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::cuboid;

    #[test]
    fn the_faces_one_box_buries_in_another_are_dropped() {
        let left = cuboid(Vec3::ZERO, Vec3::new(20.0, 20.0, 20.0));
        let right = cuboid(Vec3::new(10.0, 0.0, 0.0), Vec3::new(30.0, 20.0, 20.0));
        let surface = unburied(&[left, right], &[true, true]);
        let inside_x = |x: Scalar| (x - 10.0).abs() < 1e-4 || (x - 20.0).abs() < 1e-4;
        let walls_inside = surface
            .faces
            .iter()
            .filter(|face| {
                face.iter()
                    .all(|corner| inside_x(surface.vertices[*corner as usize].x))
            })
            .count();
        assert_eq!(
            walls_inside, 0,
            "neither box keeps its end inside the other"
        );
        assert_eq!(
            surface.faces.len(),
            24 - 4,
            "each box loses its two buried triangles"
        );
    }

    /// A box open at one end with a second pushed into the opening: the second's end inside
    /// the first is what closes it, so it stays.
    #[test]
    fn a_face_over_another_bodys_hole_is_kept() {
        let mut open = cuboid(Vec3::ZERO, Vec3::new(20.0, 20.0, 20.0));
        let at_the_end = |mesh: &Mesh, face: &[u32; 3]| {
            face.iter()
                .all(|corner| (mesh.vertices[*corner as usize].x - 20.0).abs() < 1e-4)
        };
        let ends: Vec<[u32; 3]> = open
            .faces
            .iter()
            .filter(|face| !at_the_end(&open, face))
            .copied()
            .collect();
        open.faces = ends;
        let plug = cuboid(Vec3::new(10.0, 0.0, 0.0), Vec3::new(30.0, 20.0, 20.0));
        let surface = unburied(&[open, plug], &[true, true]);
        assert_eq!(
            surface.faces.len(),
            10 + 12,
            "nothing buries in a box with no end"
        );
    }

    #[test]
    fn boxes_side_by_side_keep_every_face() {
        let left = cuboid(Vec3::ZERO, Vec3::splat(10.0));
        let right = cuboid(Vec3::new(20.0, 0.0, 0.0), Vec3::new(30.0, 10.0, 10.0));
        let surface = unburied(&[left, right], &[true, true]);
        assert_eq!(surface.faces.len(), 24);
    }
}
