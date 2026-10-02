use core_geometry::{Mesh, Scalar};

/// Faces bucketed by the Z range they span, so a plane only tests the faces near it.
pub(crate) struct ZBins {
    z_min: Scalar,
    z_max: Scalar,
    bin_height: Scalar,
    bins: Vec<Vec<u32>>,
}

/// Cap on the number of buckets. More would shorten each one, but a tall face has to be
/// listed in every bucket it spans, and that cost grows with the count.
const MAX_BINS: usize = 1024;

impl ZBins {
    /// Buckets the faces of `mesh` over `[z_min, z_max]` for a run of `planes` layers.
    ///
    /// A face outside the range is left out, so a window's index costs a window; see
    /// `docs/decisions/0066-a-stack-is-sliced-a-window-at-a-time.md`.
    pub(crate) fn build(mesh: &Mesh, z_min: Scalar, z_max: Scalar, planes: usize) -> Self {
        let count = planes.clamp(1, MAX_BINS);
        let bin_height = ((z_max - z_min) / count as Scalar).max(Scalar::MIN_POSITIVE);
        let mut bins = vec![Vec::new(); count];

        for (face, indices) in mesh.faces.iter().enumerate() {
            let Some((low, high)) = face_span(mesh, *indices) else {
                continue;
            };
            // A flat face never crosses a plane, and neither does one out of range.
            if low >= high || high < z_min || low > z_max {
                continue;
            }
            let first = bin_of(low, z_min, bin_height, count);
            let last = bin_of(high, z_min, bin_height, count);
            for bin in &mut bins[first..=last] {
                bin.push(face as u32);
            }
        }

        Self {
            z_min,
            z_max,
            bin_height,
            bins,
        }
    }

    /// Every face that can possibly cross the plane at height `z`.
    ///
    /// The top of the range reads the last bucket: a window's index spans its own planes,
    /// so the topmost plane lands exactly on the edge.
    pub(crate) fn faces_near(&self, z: Scalar) -> &[u32] {
        if self.bins.is_empty() || z < self.z_min || z > self.z_max {
            return &[];
        }
        let index = (((z - self.z_min) / self.bin_height) as usize).min(self.bins.len() - 1);
        self.bins.get(index).map_or(&[], Vec::as_slice)
    }
}

fn bin_of(z: Scalar, z_min: Scalar, bin_height: Scalar, count: usize) -> usize {
    // A negative or out-of-range height saturates on the cast, so the clamp is enough.
    (((z - z_min) / bin_height) as usize).min(count - 1)
}

fn face_span(mesh: &Mesh, indices: [u32; 3]) -> Option<(Scalar, Scalar)> {
    let mut low = Scalar::INFINITY;
    let mut high = Scalar::NEG_INFINITY;
    for index in indices {
        let z = mesh.vertices.get(index as usize)?.z;
        low = low.min(z);
        high = high.max(z);
    }
    Some((low, high))
}

#[cfg(test)]
mod tests {
    use core_geometry::Vec3;

    use super::*;

    /// Three stacked triangles, one per millimetre of height.
    fn stack() -> Mesh {
        let mut vertices = Vec::new();
        let mut faces = Vec::new();
        for step in 0..3u32 {
            let base = step as Scalar;
            vertices.extend([
                Vec3::new(0.0, 0.0, base),
                Vec3::new(1.0, 0.0, base),
                Vec3::new(0.0, 1.0, base + 1.0),
            ]);
            faces.push([step * 3, step * 3 + 1, step * 3 + 2]);
        }
        Mesh::new(vertices, faces)
    }

    #[test]
    fn a_plane_only_sees_the_faces_that_reach_it() {
        let bins = ZBins::build(&stack(), 0.0, 3.0, 3);

        // Buckets are a filter, not an answer: they may over-report at a boundary, but
        // never leave out a face the plane actually crosses.
        for (z, face) in [(0.5, 0), (1.5, 1), (2.5, 2)] {
            assert!(bins.faces_near(z).contains(&face));
            assert!(bins.faces_near(z).len() < 3);
        }
    }

    #[test]
    fn a_face_spanning_several_bins_is_listed_in_each() {
        let tall = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 3.0),
            ],
            vec![[0, 1, 2]],
        );
        let bins = ZBins::build(&tall, 0.0, 3.0, 3);

        for z in [0.5, 1.5, 2.5] {
            assert_eq!(bins.faces_near(z), &[0]);
        }
    }

    #[test]
    fn a_flat_face_is_left_out() {
        let flat = Mesh::new(
            vec![
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(0.0, 1.0, 1.0),
            ],
            vec![[0, 1, 2]],
        );
        let bins = ZBins::build(&flat, 0.0, 2.0, 2);

        assert!(bins.faces_near(1.0).is_empty());
    }

    #[test]
    fn a_plane_outside_the_bounds_sees_nothing() {
        let bins = ZBins::build(&stack(), 0.0, 3.0, 3);

        assert!(bins.faces_near(-1.0).is_empty());
        assert!(bins.faces_near(9.0).is_empty());
    }

    #[test]
    fn a_plane_on_the_top_of_the_range_still_sees_its_faces() {
        // An index built for a window spans its own planes, so the last one is the edge.
        let bins = ZBins::build(&stack(), 0.5, 2.5, 2);

        assert!(!bins.faces_near(2.5).is_empty());
    }

    #[test]
    fn a_face_the_range_misses_is_left_out() {
        let bins = ZBins::build(&stack(), 2.1, 2.9, 2);

        // Only the third triangle, which spans 2 to 3 mm, reaches that window.
        let listed: std::collections::HashSet<u32> = bins.bins.iter().flatten().copied().collect();
        assert_eq!(listed, std::collections::HashSet::from([2]));
    }

    #[test]
    fn a_face_with_a_broken_index_is_skipped() {
        let broken = Mesh::new(vec![Vec3::ZERO], vec![[0, 1, 2]]);
        let bins = ZBins::build(&broken, 0.0, 1.0, 1);

        assert!(bins.faces_near(0.5).is_empty());
    }
}
