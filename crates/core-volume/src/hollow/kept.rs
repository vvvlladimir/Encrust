use core_geometry::{Mesh, Scalar, Winding, signed_volume, split};
use rayon::prelude::*;

use super::Blocker;
use crate::drain::HoleSize;
use crate::shells::{SAMPLES, most_faces};

/// The pieces of a shell's cavity that hold resin a drain can reach, joined back into one;
/// `voxel_mm` is how far off a blocker the lattice may put a piece. See `docs/design/hollowing.md`.
pub(super) fn kept(void: Mesh, model: &Mesh, blockers: &[Blocker], voxel_mm: Scalar) -> Mesh {
    let mut pieces: Vec<(Mesh, Scalar)> = split(&void)
        .into_par_iter()
        .map(|piece| {
            let volume = signed_volume(&piece);
            (piece, volume)
        })
        .collect();
    if pieces.len() <= 1 {
        return void;
    }
    drop(void);

    pieces.sort_unstable_by(|one, other| other.1.total_cmp(&one.1));
    let least = drain_ball_mm3();
    let (mut cavity, mut islands) = (Vec::new(), Vec::new());
    for (rank, (piece, volume)) in pieces.into_iter().enumerate() {
        if volume < 0.0 && is_blocked(&piece, blockers, voxel_mm) {
            islands.push(piece);
        } else if volume > 0.0 && (rank == 0 || volume >= least) {
            cavity.push(piece);
        }
    }
    // One piece left is the cavity itself; only a second one asks what the model holds.
    if cavity.len() > 1 {
        let winding = Winding::build(model);
        let inside: Vec<bool> = cavity
            .par_iter()
            .map(|piece| is_inside(piece, model, &winding))
            .collect();
        // A wrong side only ever adds a piece: where none reads inside, the largest is the cavity.
        if inside.contains(&true) {
            let mut inside = inside.into_iter();
            cavity.retain(|_| inside.next() == Some(true));
        } else {
            cavity.truncate(1);
        }
    }

    let mut kept = Mesh::default();
    for piece in cavity.iter().chain(&islands) {
        kept.append(piece);
    }
    kept
}

/// What a ball as wide as the hole a drain is drilled by default holds, in cubic millimetres.
fn drain_ball_mm3() -> Scalar {
    let radius = HoleSize::default().diameter_mm / 2.0;
    4.0 / 3.0 * std::f32::consts::PI * radius.powi(3)
}

/// Whether `piece` stands on the surface of one of `blockers`, so a blocker carved it.
fn is_blocked(piece: &Mesh, blockers: &[Blocker], voxel_mm: Scalar) -> bool {
    piece
        .vertices
        .iter()
        .step_by((piece.vertices.len() / SAMPLES).max(1))
        .any(|vertex| {
            blockers
                .iter()
                .any(|blocker| blocker.depth_at(*vertex).abs() <= voxel_mm)
        })
}

/// Whether most of `piece` lies inside `model`, by the model's winding number at its faces.
fn is_inside(piece: &Mesh, model: &Mesh, winding: &Winding) -> bool {
    most_faces(piece, |centroid, _| winding.is_inside(model, centroid))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Vec3, ball, cuboid};

    use crate::drain::inward;
    use crate::fixtures::joined;

    #[test]
    fn only_the_cavity_inside_the_model_is_kept() {
        let model = cuboid(Vec3::ZERO, Vec3::splat(20.0));
        let cavity = cuboid(Vec3::splat(2.0), Vec3::splat(18.0));
        let outside = cuboid(Vec3::new(30.0, 0.0, 0.0), Vec3::new(35.0, 5.0, 5.0));
        let island = inward(&cuboid(Vec3::splat(8.0), Vec3::splat(10.0)));
        let pocket = cuboid(Vec3::splat(0.5), Vec3::splat(1.5));
        let sheet = cuboid(Vec3::new(5.0, 5.0, 5.0), Vec3::new(5.0, 8.0, 8.0));

        let kept = kept(
            joined(&[cavity.clone(), outside, island, pocket, sheet]),
            &model,
            &[],
            0.2,
        );
        assert_eq!(kept.faces.len(), cavity.faces.len());
        assert_eq!(kept.aabb(), cavity.aabb(), "what is left is the 16 mm box");
    }

    #[test]
    fn the_largest_piece_stays_when_none_reads_inside_the_model() {
        let model = cuboid(Vec3::ZERO, Vec3::splat(20.0));
        let larger = cuboid(Vec3::new(30.0, 0.0, 0.0), Vec3::new(40.0, 10.0, 10.0));
        let smaller = cuboid(Vec3::new(50.0, 0.0, 0.0), Vec3::new(55.0, 5.0, 5.0));
        let kept = kept(joined(&[smaller, larger.clone()]), &model, &[], 0.2);
        assert_eq!(kept.aabb(), larger.aabb(), "the 10 mm box is what is left");
    }

    #[test]
    fn a_cavity_smaller_than_a_drain_ball_is_kept_when_it_is_the_only_one() {
        let model = cuboid(Vec3::ZERO, Vec3::splat(4.0));
        let cavity = cuboid(Vec3::splat(1.5), Vec3::splat(2.5));
        let island = inward(&cuboid(Vec3::splat(1.9), Vec3::splat(2.1)));
        let kept = kept(joined(&[cavity.clone(), island]), &model, &[], 0.05);
        assert_eq!(
            kept.aabb(),
            cavity.aabb(),
            "the 1 mm box is the whole cavity"
        );
    }

    #[test]
    fn solid_a_blocker_left_in_the_cavity_stays() {
        let model = cuboid(Vec3::ZERO, Vec3::splat(20.0));
        let cavity = cuboid(Vec3::splat(2.0), Vec3::splat(18.0));
        let island = inward(&ball(Vec3::splat(10.0), 2.0, 16, 16));
        let blocker = Blocker::ball(Vec3::splat(10.0), 2.0);
        let kept = kept(
            joined(&[cavity.clone(), island.clone()]),
            &model,
            &[blocker],
            0.2,
        );
        assert_eq!(kept.faces.len(), cavity.faces.len() + island.faces.len());
    }
}
