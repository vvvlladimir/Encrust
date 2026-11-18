use rayon::prelude::*;

use crate::topology::{edge_groups, edge_uses};
use crate::{FastMap, Mesh, Scalar, Vec3, cell_of_point};

/// One face as the map reads it. A degenerate face has no area and no normal.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Sample {
    pub centroid: Vec3,
    pub normal: Vec3,
    pub area_mm2: Scalar,
    /// The mean curvature integrated over the face, millimetres: positive where it bulges.
    pub curvature_mm: Scalar,
}

/// Every face of `mesh` with its share of the mean curvature of the edges it has: half the
/// dihedral angle times the length of each edge it shares with one other face.
pub(super) fn samples(mesh: &Mesh) -> Vec<Sample> {
    let mut samples: Vec<Sample> = (0..mesh.faces.len())
        .into_par_iter()
        .map(|face| {
            let Some(triangle) = mesh.triangle(face) else {
                return Sample::default();
            };
            let scaled = triangle.normal_unnormalized();
            let area_mm2 = scaled.length() / 2.0;
            Sample {
                centroid: (triangle.a + triangle.b + triangle.c) / 3.0,
                normal: scaled.normalize_or_zero(),
                area_mm2,
                curvature_mm: 0.0,
            }
        })
        .collect();
    let uses = edge_uses(mesh);
    let bends: Vec<(u32, u32, Scalar)> = edge_groups(&uses)
        .filter(|group| group.len() == 2)
        .map(|pair| {
            let (from, to) = pair[0].edge;
            let length = (mesh.vertices[to as usize] - mesh.vertices[from as usize]).length();
            let (a, b) = (pair[0].face, pair[1].face);
            (
                a,
                b,
                bend(&samples[a as usize], &samples[b as usize]) * length,
            )
        })
        .collect();
    for (a, b, bend_mm) in bends {
        // An edge's mean curvature is half its dihedral angle times its length, split
        // evenly between the two faces that share it.
        samples[a as usize].curvature_mm += bend_mm / 4.0;
        samples[b as usize].curvature_mm += bend_mm / 4.0;
    }
    samples
}

/// The angle between two faces' normals, radians, positive where the surface bulges out
/// between them and negative where it folds in.
fn bend(a: &Sample, b: &Sample) -> Scalar {
    if a.area_mm2 <= 0.0 || b.area_mm2 <= 0.0 {
        return 0.0;
    }
    let angle = a.normal.dot(b.normal).clamp(-1.0, 1.0).acos();
    if (b.centroid - a.centroid).dot(a.normal) > 0.0 {
        -angle
    } else {
        angle
    }
}

/// What the faces in one cube of the grid add up to.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Cell {
    pub area_mm2: Scalar,
    /// The centroids summed weighted by area: over the area, where the cell's surface is.
    pub centroid_sum: Vec3,
    pub curvature_mm: Scalar,
}

impl Cell {
    pub fn centroid(&self) -> Vec3 {
        self.centroid_sum / self.area_mm2.max(Scalar::EPSILON)
    }

    fn add(&mut self, other: &Self) {
        self.area_mm2 += other.area_mm2;
        self.centroid_sum += other.centroid_sum;
        self.curvature_mm += other.curvature_mm;
    }
}

/// The largest face of the faces in one cube that face one way: the point the cube is
/// looked out from.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Outlook {
    pub point: Vec3,
    pub normal: Vec3,
    area_mm2: Scalar,
}

/// The surface gathered into the cubes of a grid `cell_mm` across, sparse: only cubes the
/// surface passes through are kept.
pub(super) struct Grid {
    pub cell_mm: Scalar,
    pub cells: FastMap<[i32; 3], Cell>,
    /// The cubes again, split by which of the six axis directions a face is nearest.
    pub outlooks: FastMap<[i32; 4], Outlook>,
}

impl Grid {
    pub fn of(samples: &[Sample], cell_mm: Scalar) -> Self {
        let (cells, outlooks) = samples
            .par_iter()
            .filter(|sample| sample.area_mm2 > 0.0)
            .fold(
                || (FastMap::default(), FastMap::default()),
                |(mut cells, mut outlooks), sample| {
                    let cell = Cell {
                        area_mm2: sample.area_mm2,
                        centroid_sum: sample.centroid * sample.area_mm2,
                        curvature_mm: sample.curvature_mm,
                    };
                    let at = cell_of_point(sample.centroid, cell_mm);
                    cells.entry(at).or_insert_with(Cell::default).add(&cell);
                    let outlook = Outlook {
                        point: sample.centroid,
                        normal: sample.normal,
                        area_mm2: sample.area_mm2,
                    };
                    keep_larger(&mut outlooks, facing_key(at, sample.normal), outlook);
                    (cells, outlooks)
                },
            )
            .reduce(
                || (FastMap::default(), FastMap::default()),
                |(mut cells, mut outlooks), (other_cells, other_outlooks)| {
                    for (at, cell) in other_cells {
                        cells.entry(at).or_insert_with(Cell::default).add(&cell);
                    }
                    for (at, outlook) in other_outlooks {
                        keep_larger(&mut outlooks, at, outlook);
                    }
                    (cells, outlooks)
                },
            );
        Self {
            cell_mm,
            cells,
            outlooks,
        }
    }

    pub fn key_of(&self, at: Vec3) -> [i32; 3] {
        cell_of_point(at, self.cell_mm)
    }

    pub fn facing_key_of(&self, sample: &Sample) -> [i32; 4] {
        facing_key(self.key_of(sample.centroid), sample.normal)
    }
}

/// A cube and the one of the six axis directions `normal` is nearest, so that the two sides
/// of a wall thinner than a cube are looked out from apart.
fn facing_key([x, y, z]: [i32; 3], normal: Vec3) -> [i32; 4] {
    let magnitude = normal.abs();
    let (axis, along) = if magnitude.x >= magnitude.y && magnitude.x >= magnitude.z {
        (0, normal.x)
    } else if magnitude.y >= magnitude.z {
        (1, normal.y)
    } else {
        (2, normal.z)
    };
    [x, y, z, axis * 2 + i32::from(along < 0.0)]
}

fn keep_larger(outlooks: &mut FastMap<[i32; 4], Outlook>, at: [i32; 4], outlook: Outlook) {
    let kept = outlooks.entry(at).or_insert(outlook);
    if outlook.area_mm2 > kept.area_mm2 {
        *kept = outlook;
    }
}
