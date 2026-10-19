use core_csg::CsgError;
use core_geometry::{FastMap, FastSet, Mesh, Plane, Scalar, Vec3, cut, diagnose};
use rayon::prelude::*;

use super::{Boolean, exact};

/// How far under the lowest point of the others the model is cut, in millimetres: clear of
/// every face the engine adds where they meet it, so the cut and its cap pass through untouched.
const CLEARANCE_MM: Scalar = 0.5;

/// A point this close to the plane is on the cut, millimetres: the cut's own band.
const ON_CUT_MM: Scalar = 1e-4;

/// How much further down the plane is tried again when the engine refuses the side cut off,
/// millimetres: a model vertex lying in the cut can leave its cap folded, and another height
/// a little off may miss it.
const STEPS_MM: [Scalar; 2] = [0.0, 0.0137];

/// Past this share of the model's vertices on the side the others stand on, cutting it
/// saves too little to be worth it.
const MOST_ABOVE: f32 = 0.5;

/// Why the model went through the engine whole instead.
#[derive(Debug)]
pub(super) enum Whole {
    /// The others stand over too much of the model, or over none of it.
    NotLocal,
    /// The cut left a loop open, or the stitch did not close.
    Seam,
    /// The engine refused the side that was cut off; the whole model is asked again, and
    /// answers for itself.
    Engine,
}

/// `models` combined by `op`, the first cut by a plane square to `toward` under all the
/// others, so only its side they stand on goes through the engine; see
/// `docs/design/csg.md`.
pub(super) fn local(models: &[Mesh], op: Boolean, toward: Vec3) -> Result<Mesh, Whole> {
    let (model, others) = models.split_first().ok_or(Whole::NotLocal)?;
    // A torn model is refused whole, and shells crossing are unioned whole first: cut, the
    // side below would keep what the whole cut refuses or counts once.
    let report = diagnose(model);
    if !(report.is_sound() && report.unbalanced_edges == 0 && report.shells == 1) {
        return Err(Whole::NotLocal);
    }
    let normal = toward.try_normalize().ok_or(Whole::NotLocal)?;
    let lowest = others
        .iter()
        .flat_map(|other| other.vertices.iter())
        .map(|vertex| vertex.dot(normal))
        .fold(Scalar::MAX, Scalar::min);
    let mut refused = Whole::NotLocal;
    for step in STEPS_MM {
        let plane =
            plane_under(model, normal, lowest - CLEARANCE_MM - step).ok_or(Whole::NotLocal)?;
        match near_plane(model, others, op, plane) {
            Err(Whole::Engine) => refused = Whole::Engine,
            done => return done,
        }
    }
    Err(refused)
}

/// `models` combined by `op` with the model cut at `plane`.
fn near_plane(model: &Mesh, others: &[Mesh], op: Boolean, plane: Plane) -> Result<Mesh, Whole> {
    let halves = cut(model, plane);
    if halves.open_loops > 0 {
        return Err(Whole::Seam);
    }
    if halves.above.faces.is_empty() {
        return Err(Whole::NotLocal);
    }
    let mut near = vec![halves.above];
    near.extend(others.iter().cloned());
    let combined = exact(&near, op).map_err(|_: CsgError| Whole::Engine)?;
    stitched(&halves.below, &combined, plane)
}

/// The plane square to `normal` at `height_mm`; `None` when the side above it holds most
/// of `mesh`.
fn plane_under(mesh: &Mesh, normal: Vec3, height_mm: Scalar) -> Option<Plane> {
    let above = mesh
        .vertices
        .par_iter()
        .filter(|vertex| vertex.dot(normal) > height_mm)
        .count();
    (above as f32 <= MOST_ABOVE * mesh.vertices.len() as f32).then_some(Plane {
        point: normal * height_mm,
        normal,
    })
}

/// The lower half without its cap, and `upper` — the cut-off side as the engine answered
/// it — without its own, joined along the cut where every point of one is a point of the
/// other and every edge of the one is walked back by the other.
fn stitched(lower: &Mesh, upper: &Mesh, plane: Plane) -> Result<Mesh, Whole> {
    let on_cut = |point: Vec3| (point - plane.point).dot(plane.normal).abs() <= ON_CUT_MM;
    // The upper half holds no face of the model's own lying in the cut: those the cut puts
    // below. So every face of it there is its cap, and the cap's corners are the seam.
    let upper_cap = |face: &[u32; 3]| {
        face.iter()
            .all(|vertex| on_cut(upper.vertices[*vertex as usize]))
    };
    let mut seam: FastMap<[u32; 3], u32> = FastMap::default();
    for (index, vertex) in lower.vertices.iter().enumerate() {
        if on_cut(*vertex) {
            seam.insert(vertex.to_array().map(f32::to_bits), index as u32);
        }
    }
    let mut vertices = lower.vertices.clone();
    let mut remap = vec![u32::MAX; upper.vertices.len()];
    for face in upper.faces.iter().filter(|face| upper_cap(face)) {
        for vertex in face {
            let at = upper.vertices[*vertex as usize]
                .to_array()
                .map(f32::to_bits);
            remap[*vertex as usize] = *seam.get(&at).ok_or(Whole::Seam)?;
        }
    }
    let cut_line: FastSet<u32> = remap.iter().copied().filter(|at| *at != u32::MAX).collect();
    for (index, vertex) in upper.vertices.iter().enumerate() {
        if remap[index] == u32::MAX {
            remap[index] = vertices.len() as u32;
            vertices.push(*vertex);
        }
    }
    let upper_faces: Vec<[u32; 3]> = upper
        .faces
        .iter()
        .filter(|face| !upper_cap(face))
        .map(|face| face.map(|vertex| remap[vertex as usize]))
        .collect();
    // The lower cap is the faces facing up out of it with every corner on the cut line.
    let lower_cap = |face: &[u32; 3]| {
        face.iter().all(|vertex| cut_line.contains(vertex))
            && facing(lower, face).dot(plane.normal) > 0.0
    };
    let lower_faces: Vec<[u32; 3]> = lower
        .faces
        .par_iter()
        .filter(|face| !lower_cap(face))
        .copied()
        .collect();
    if !closes(&lower_faces, &upper_faces, &cut_line) {
        return Err(Whole::Seam);
    }
    let mut faces = lower_faces;
    faces.extend(upper_faces);
    Ok(Mesh::new(vertices, faces))
}

fn facing(mesh: &Mesh, [a, b, c]: &[u32; 3]) -> Vec3 {
    let at = |vertex: u32| mesh.vertices[vertex as usize];
    (at(*b) - at(*a)).cross(at(*c) - at(*a))
}

/// Whether every edge along the cut is walked once by a face of `lower` and once back by
/// one of `upper`, which is what leaves the two closed as one.
fn closes(lower: &[[u32; 3]], upper: &[[u32; 3]], cut_line: &FastSet<u32>) -> bool {
    let along = |faces: &[[u32; 3]]| -> Vec<(u32, u32)> {
        faces
            .par_iter()
            .flat_map_iter(|[a, b, c]| [(*a, *b), (*b, *c), (*c, *a)])
            .filter(|(a, b)| cut_line.contains(a) && cut_line.contains(b))
            .collect()
    };
    let mut down: Vec<(u32, u32)> = along(lower);
    let mut up: Vec<(u32, u32)> = along(upper).into_iter().map(|(a, b)| (b, a)).collect();
    down.sort_unstable();
    up.sort_unstable();
    !down.is_empty() && down.windows(2).all(|pair| pair[0] != pair[1]) && down == up
}

#[cfg(test)]
mod tests {
    use core_geometry::{cuboid, diagnose, signed_volume};

    use super::*;

    /// A 10 mm square bar 100 mm tall, and two 4 mm blocks standing a millimetre out of its
    /// top and a millimetre into it.
    fn lettered_bar() -> Vec<Mesh> {
        vec![
            cuboid(Vec3::ZERO, Vec3::new(10.0, 10.0, 100.0)),
            cuboid(Vec3::new(1.0, 3.0, 99.0), Vec3::new(5.0, 7.0, 101.0)),
            cuboid(Vec3::new(6.0, 3.0, 99.0), Vec3::new(8.0, 7.0, 101.0)),
        ]
    }

    #[test]
    fn blocks_on_a_bar_are_combined_through_its_top_alone_and_the_bar_stays_closed() {
        for (op, expected) in [
            (Boolean::Union, 10_000.0 + 16.0 + 8.0),
            (Boolean::Difference, 10_000.0 - 16.0 - 8.0),
        ] {
            let mesh = local(&lettered_bar(), op, Vec3::Z).expect("the top is cut off and joined");
            let report = diagnose(&mesh);
            assert!(
                report.is_sound() && report.unbalanced_edges == 0,
                "{op:?}: the halves join closed; {report:?}"
            );
            let got = signed_volume(&mesh);
            assert!(
                (got - expected).abs() < 1e-2,
                "{op:?}: a 10000 mm³ bar and a millimetre of 16 and 8 mm² blocks; got {got}"
            );
        }
    }

    #[test]
    fn others_standing_over_most_of_the_model_leave_it_whole() {
        assert!(matches!(
            local(&lettered_bar(), Boolean::Union, -Vec3::Z),
            Err(Whole::NotLocal)
        ));
    }
}
