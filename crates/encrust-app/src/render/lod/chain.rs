//! The lighter levels of a heavy mesh, each about half the one before, and the cache that
//! builds them on a worker the first time a mesh is drawn.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Weak};

use core_geometry::{Bvh, FastMap, Mesh, Vec3};
use meshoptimizer_rs::{
    Error, Positions, SimplifyOptions, SimplifySettings, Workspace, simplify, simplify_scale,
    simplify_sloppy,
};

/// A mesh with fewer faces than this is always drawn whole: it costs the card little.
pub const MIN_FACES: usize = 20_000;
/// The chain stops once a level is this light.
const FEWEST_FACES: usize = 2_000;
/// Faces of each level whose middles are measured against the mesh: the simplifier's own
/// error counts its corners only, and on a curved surface misses a face sinking in.
const SAMPLES: usize = 4096;
/// A level keeping more than this share of the one before cannot go much lower without
/// tearing, and ends the chain.
const STALLED: f32 = 0.8;

/// One lighter way of drawing a mesh, and how far its surface may stand from the mesh's
/// own, mm in the mesh's frame.
#[derive(Debug, Clone)]
pub struct Level {
    pub mesh: Arc<Mesh>,
    pub error_mm: f32,
}

/// Builds the levels of `mesh`, finest first, handing each to `landed` as it is made.
/// A mesh under `MIN_FACES` has none.
pub fn chain(mesh: &Mesh, mut landed: impl FnMut(Level)) -> Result<(), Error> {
    if mesh.faces.len() < MIN_FACES {
        return Ok(());
    }
    let positions: Vec<[f32; 3]> = mesh
        .vertices
        .iter()
        .map(|vertex| vertex.to_array())
        .collect();
    let extent_mm = simplify_scale(Positions::from_packed(&positions))?;
    let mut indices: Vec<u32> = mesh
        .faces
        .iter()
        .filter(|face| {
            face.iter()
                .all(|&corner| (corner as usize) < positions.len())
        })
        .flatten()
        .copied()
        .collect();
    let mut workspace = Workspace::default();
    let surface = Bvh::build(mesh);
    let mut error_mm: f32 = 0.0;
    while indices.len() / 3 > FEWEST_FACES {
        let target = indices.len() / 2;
        let (lighter, step_mm) = halved(&indices, &positions, extent_mm, target, &mut workspace)?;
        if lighter.len() as f32 > indices.len() as f32 * STALLED {
            break;
        }
        let level = compacted(&lighter, &mesh.vertices);
        // Each level is simplified from the one before, so their errors add up.
        error_mm = (error_mm + step_mm).max(sunk_mm(&level, mesh, &surface));
        landed(Level {
            mesh: Arc::new(level),
            error_mm,
        });
        indices = lighter;
    }
    Ok(())
}

/// `indices` simplified to about `target` of them by collapsing edges, or where that
/// stalls on a surface it may not tear, by merging corners on a grid.
fn halved(
    indices: &[u32],
    positions: &[[f32; 3]],
    extent_mm: f32,
    target: usize,
    workspace: &mut Workspace,
) -> Result<(Vec<u32>, f32), Error> {
    let settings = SimplifySettings {
        target_index_count: target,
        target_error: extent_mm,
        options: SimplifyOptions::ERROR_ABSOLUTE | SimplifyOptions::SPARSE,
    };
    let collapsed = simplify(
        indices,
        Positions::from_packed(positions),
        settings,
        workspace,
    )?;
    if collapsed.indices.len() as f32 <= indices.len() as f32 * STALLED {
        return Ok((collapsed.indices, collapsed.error));
    }
    let merged = simplify_sloppy(
        indices,
        Positions::from_packed(positions),
        None,
        target,
        1.0,
        workspace,
    )?;
    // The grid's error is relative to the whole mesh's extent.
    Ok((merged.indices, merged.error * extent_mm))
}

/// The farthest the middle of a face of `level` stands from `mesh`, over faces spread
/// evenly through it, mm.
fn sunk_mm(level: &Mesh, mesh: &Mesh, surface: &Bvh) -> f32 {
    let stride = (level.faces.len() / SAMPLES).max(1);
    (0..level.faces.len())
        .step_by(stride)
        .filter_map(|face| level.triangle(face))
        .filter_map(|triangle| surface.closest(mesh, (triangle.a + triangle.b + triangle.c) / 3.0))
        .map(|closest| closest.distance)
        .fold(0.0, f32::max)
}

/// A mesh of the triangles `indices` names, keeping only the corners they use.
fn compacted(indices: &[u32], vertices: &[Vec3]) -> Mesh {
    let mut remap = vec![u32::MAX; vertices.len()];
    let mut kept = Vec::new();
    let mut corner = |index: u32| {
        let slot = &mut remap[index as usize];
        if *slot == u32::MAX {
            *slot = kept.len() as u32;
            kept.push(vertices[index as usize]);
        }
        *slot
    };
    let faces = indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&[a, b, c]| [corner(a), corner(b), corner(c)])
        .collect();
    Mesh::new(kept, faces)
}

/// The levels of every heavy mesh the viewport has drawn, built on a worker the first
/// time a mesh is asked for and let go with it.
pub struct Lods {
    built: FastMap<usize, Built>,
    landed: Receiver<(usize, Level)>,
    sender: Sender<(usize, Level)>,
}

/// One mesh's entry: the mesh itself, held weakly so the cache never keeps it alive, and
/// the levels that have landed so far, finest first.
struct Built {
    source: Weak<Mesh>,
    levels: Vec<Level>,
}

impl Default for Lods {
    fn default() -> Self {
        let (sender, landed) = mpsc::channel();
        Self {
            built: FastMap::default(),
            landed,
            sender,
        }
    }
}

impl Lods {
    /// Takes in the levels built since the last frame and drops the entries of meshes
    /// nothing holds any more. Returns whether a level landed, which is worth a frame.
    pub fn poll(&mut self) -> bool {
        let mut landed = false;
        while let Ok((key, level)) = self.landed.try_recv() {
            if let Some(entry) = self.built.get_mut(&key) {
                entry.levels.push(level);
                landed = true;
            }
        }
        self.built
            .retain(|_, entry| entry.source.strong_count() > 0);
        landed
    }

    /// The levels of `mesh` built so far, finest first. The first time a mesh is asked
    /// for its chain is started on a worker, and it is drawn whole until levels land.
    pub fn of(&mut self, mesh: &Arc<Mesh>) -> &[Level] {
        if mesh.faces.len() < MIN_FACES {
            return &[];
        }
        let key = Arc::as_ptr(mesh) as usize;
        let fresh = self
            .built
            .get(&key)
            .is_none_or(|entry| !entry.source.ptr_eq(&Arc::downgrade(mesh)));
        if fresh {
            self.start(key, mesh);
        }
        &self.built[&key].levels
    }

    fn start(&mut self, key: usize, mesh: &Arc<Mesh>) {
        self.built.insert(
            key,
            Built {
                source: Arc::downgrade(mesh),
                levels: Vec::new(),
            },
        );
        let (source, sender) = (Arc::clone(mesh), self.sender.clone());
        crate::job::spawn(move || {
            let built = chain(&source, |level| {
                // The window closing before the chain is done leaves nobody to take it.
                let _ = sender.send((key, level));
            });
            if let Err(error) = built {
                tracing::warn!(%error, "a model's lighter copies could not be built");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::lod::tests::ball;

    #[test]
    fn a_light_mesh_has_no_levels() {
        let mut levels = Vec::new();
        chain(&ball(10.0, 40, 40), |level| levels.push(level)).expect("a closed ball");
        assert!(levels.is_empty(), "3 120 faces are drawn whole");
    }

    #[test]
    fn each_level_halves_the_one_before_and_keeps_the_ball_round() {
        let heavy = ball(20.0, 200, 200);
        let mut levels = Vec::new();
        chain(&heavy, |level| levels.push(level)).expect("a closed ball");
        assert!(levels.len() >= 4, "79 600 faces halve at least four times");
        let mut faces = heavy.faces.len();
        let mut error_mm = 0.0;
        for level in &levels {
            let kept = level.mesh.faces.len();
            assert!(
                kept * 10 <= faces * 6 && kept * 10 >= faces * 3,
                "{kept} faces after {faces}, about half"
            );
            assert!(level.error_mm >= error_mm, "errors only add up");
            // Corners are the ball's own, so the error shows in the middle of a face.
            let off_mm = (0..level.mesh.faces.len())
                .filter_map(|face| level.mesh.triangle(face))
                .map(|triangle| 20.0 - ((triangle.a + triangle.b + triangle.c) / 3.0).length())
                .fold(0.0, f32::max);
            assert!(
                off_mm <= 1.5 * level.error_mm + 1e-3,
                "every face near the level's {} mm of the 20 mm sphere, {off_mm} mm in",
                level.error_mm
            );
            (faces, error_mm) = (kept, level.error_mm);
        }
    }

    #[test]
    fn a_level_keeps_only_the_corners_it_uses() {
        let vertices = [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z];
        let mesh = compacted(&[3, 1, 2], &vertices);
        assert_eq!(mesh.vertices, [Vec3::Z, Vec3::X, Vec3::Y]);
        assert_eq!(mesh.faces, [[0, 1, 2]]);
    }
}
