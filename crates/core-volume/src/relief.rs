use core_geometry::{Bvh, Heightmap, Mesh, Scalar, UvMap};
use serde::{Deserialize, Serialize};

use crate::build::{DEFAULT_BUDGET_BYTES, FieldSettings, build};
use crate::cancel::Cancel;
use crate::csg::assemble;
use crate::error::VolumeError;
use crate::extract::extract;
use crate::hollow::{BAND_VOXELS, COARSENINGS, lattice_mm};
use crate::sign::SignMode;

/// How far the texture may move the surface, what it is cut on, and what the field may
/// cost while it is.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ReliefSettings {
    /// How far the surface moves where the texture is white, in millimetres. Positive
    /// raises the relief out of the model, negative sinks it in.
    pub amplitude_mm: Scalar,
    /// How fine the lattice is, from 0 to 1, as everywhere else in this crate.
    pub precision: Scalar,
    pub budget_bytes: usize,
}

impl Default for ReliefSettings {
    fn default() -> Self {
        Self {
            amplitude_mm: 0.5,
            precision: 0.5,
            budget_bytes: DEFAULT_BUDGET_BYTES,
        }
    }
}

impl ReliefSettings {
    /// The lattice the relief is cut on, given the model's surface area.
    ///
    /// The amplitude stands in for a wall: it is the smallest thing the field has to
    /// carry, so it is what decides how fine the lattice has to be.
    pub fn voxel_mm(&self, area_mm2: Scalar) -> Scalar {
        lattice_mm(self.amplitude_mm.abs(), self.precision, area_mm2)
    }
}

/// A model with its texture pressed in, and what that cost.
#[derive(Debug, Clone, PartialEq)]
pub struct Relief {
    pub mesh: Mesh,
    /// The lattice the relief came out on, in millimetres.
    pub voxel_mm: Scalar,
    /// Whether the budget made that lattice coarser than precision asked for, which is
    /// the one thing that silently costs the relief its detail.
    pub coarsened: bool,
}

/// Presses a texture into `mesh`, as geometry rather than as exposure.
///
/// Every lattice point of the field is moved by the height its own nearest surface point
/// reads out of the texture, and the surface is meshed again where that leaves it, so the
/// relief needs no subdivision of the model it is pressed into; see
/// `docs/design/volume.md`.
pub fn press(
    mesh: &Mesh,
    bvh: &Bvh,
    uvs: &UvMap,
    heights: &[Heightmap],
    settings: &ReliefSettings,
) -> Result<Relief, VolumeError> {
    if mesh.aabb().is_none() {
        return Err(VolumeError::EmptyMesh);
    }
    if !settings.amplitude_mm.is_finite() || settings.amplitude_mm == 0.0 {
        return Err(VolumeError::BadAmplitude(settings.amplitude_mm));
    }
    // A map covering part of the mesh is honest — what it does not cover does not move —
    // but one indexed by a different face list is not the mesh's map at all.
    if uvs.len() != mesh.faces.len() || uvs.mapped() == 0 {
        return Err(VolumeError::UnmappedMesh {
            faces: mesh.faces.len(),
            mapped: uvs.mapped(),
        });
    }
    // A model textured by material carries one image per material, and a face names its
    // own: a map naming an image that was not handed over has nothing to read.
    if uvs.textures() > heights.len() {
        return Err(VolumeError::MissingTexture {
            wanted: uvs.textures(),
            given: heights.len(),
        });
    }

    let asked_mm = settings.voxel_mm(mesh.surface_area());
    let mut voxel_mm = asked_mm;
    let carried = |voxel_mm: Scalar| {
        move |mesh| Relief {
            mesh,
            voxel_mm,
            coarsened: voxel_mm > asked_mm,
        }
    };
    for _ in 0..COARSENINGS {
        match pressed(mesh, bvh, uvs, heights, settings, voxel_mm) {
            // The lattice the field priced is the one that fits, taken a twentieth coarser
            // still, the way a hollowing run steps back.
            Err(VolumeError::TooFine { fits_at_mm, .. }) => voxel_mm = fits_at_mm * 1.05,
            other => return other.map(carried(voxel_mm)),
        }
    }
    pressed(mesh, bvh, uvs, heights, settings, voxel_mm).map(carried(voxel_mm))
}

/// One attempt at a given lattice.
fn pressed(
    mesh: &Mesh,
    bvh: &Bvh,
    uvs: &UvMap,
    heights: &[Heightmap],
    settings: &ReliefSettings,
    voxel_mm: Scalar,
) -> Result<Mesh, VolumeError> {
    let reach_mm = settings.amplitude_mm.abs();
    let field = build(
        mesh,
        bvh,
        &FieldSettings {
            voxel_mm,
            // The displaced surface has to stay inside the band it was built in, so the
            // band carries the amplitude on top of what marching the cells needs.
            band_voxels: reach_mm / voxel_mm + BAND_VOXELS,
            iso_mm: 0.0,
            sign: SignMode::Auto,
            clip: None,
            budget_bytes: settings.budget_bytes,
        },
        Cancel::never(),
    )?;

    let grid = field.grid();
    let band_mm = field.band_mm();
    let tiles = field.tile_keys().collect();
    let displaced = assemble(grid, band_mm - reach_mm, tiles, |voxel| {
        let value = field.value(voxel);
        // Past the band the value is clamped anyway, and no displacement of it can reach
        // back across zero, so the nearest-point query is not worth making.
        if value.abs() >= band_mm {
            return value;
        }
        let point = grid.position(voxel);
        let Some(found) = bvh.closest(mesh, point) else {
            return value;
        };
        let Some((texture, uv)) = uvs.at(mesh, found.face, found.point) else {
            return value;
        };
        let Some(heights) = heights.get(texture) else {
            return value;
        };
        value - settings.amplitude_mm * heights.sample(uv)
    });

    Ok(extract(&displaced, Cancel::never()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_geometry::{Aabb, Mapping, Vec2, Vec3};

    /// A closed cube of `side` millimetres standing on the origin, with every face's UVs
    /// taken from where its corners stand in X and Y.
    fn cube(side: Scalar) -> (Mesh, UvMap) {
        let corners = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(side, 0.0, 0.0),
            Vec3::new(side, side, 0.0),
            Vec3::new(0.0, side, 0.0),
            Vec3::new(0.0, 0.0, side),
            Vec3::new(side, 0.0, side),
            Vec3::new(side, side, side),
            Vec3::new(0.0, side, side),
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];

        let mesh = Mesh::new(corners.to_vec(), faces);
        let planar = |vertex: Vec3| Vec2::new(vertex.x / side, vertex.y / side);
        let uvs = UvMap::whole(
            mesh.faces
                .iter()
                .map(|face| face.map(|index| planar(corners[index as usize])))
                .collect(),
        );
        (mesh, uvs)
    }

    fn flat(height: Scalar) -> Heightmap {
        Heightmap::new(1, 1, vec![height]).expect("one sample over one pixel")
    }

    /// Black over the left half of the unit square, white over the right, with a pixel
    /// either side of each boundary so that both halves have a plateau of their own.
    fn half() -> Heightmap {
        Heightmap::new(4, 1, vec![0.0, 0.0, 1.0, 1.0]).expect("four samples over four pixels")
    }

    fn pressed_cube(heights: &Heightmap, amplitude_mm: Scalar) -> Mesh {
        let (mesh, uvs) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        press(
            &mesh,
            &bvh,
            &uvs,
            std::slice::from_ref(heights),
            &ReliefSettings {
                amplitude_mm,
                precision: 0.5,
                budget_bytes: DEFAULT_BUDGET_BYTES,
            },
        )
        .expect("a mapped cube can be pressed")
        .mesh
    }

    fn bounds(mesh: &Mesh) -> Aabb {
        mesh.aabb().expect("the pressed mesh has faces")
    }

    /// What the lattice itself can promise: marching the cells places the surface within
    /// half a voxel, and `ReliefSettings` cuts a 0.4 mm amplitude at 0.13 mm.
    const TOLERANCE_MM: Scalar = 0.1;

    #[test]
    fn a_white_texture_moves_the_whole_surface_out() {
        let grown = bounds(&pressed_cube(&flat(1.0), 0.4));
        assert!(
            (grown.maxs.z - 10.4).abs() < TOLERANCE_MM && (grown.mins.z + 0.4).abs() < TOLERANCE_MM,
            "got {grown:?}, expected a 10 mm cube grown 0.4 mm on every side"
        );
    }

    #[test]
    fn a_negative_amplitude_sinks_the_surface_in() {
        let shrunk = bounds(&pressed_cube(&flat(1.0), -0.4));
        assert!(
            (shrunk.maxs.z - 9.6).abs() < TOLERANCE_MM,
            "got {shrunk:?}, expected a 10 mm cube taken 0.4 mm in on every side"
        );
    }

    #[test]
    fn a_black_texture_leaves_the_surface_where_it_was() {
        let same = bounds(&pressed_cube(&flat(0.0), 0.4));
        assert!(
            (same.maxs.z - 10.0).abs() < TOLERANCE_MM && same.mins.z.abs() < TOLERANCE_MM,
            "got {same:?}, expected the 10 mm cube unmoved"
        );
    }

    #[test]
    fn half_a_texture_moves_and_half_stays() {
        let mesh = pressed_cube(&half(), 0.4);
        // Inside a plateau of the texture, away from both the boundary between the halves
        // and the wrap at the edges of the square.
        let top = |from: Scalar, to: Scalar| {
            mesh.vertices
                .iter()
                .filter(|vertex| (from..to).contains(&vertex.x))
                .map(|vertex| vertex.z)
                .fold(Scalar::MIN, Scalar::max)
        };
        let white = top(6.5, 8.5);
        let black = top(1.5, 3.5);
        assert!(
            (white - 10.4).abs() < TOLERANCE_MM && (black - 10.0).abs() < TOLERANCE_MM,
            "white half topped out at {white} and black at {black}, expected 10.4 and 10.0"
        );
    }

    #[test]
    fn a_pressed_solid_is_still_a_solid() {
        let pressed = pressed_cube(&flat(1.0), 0.4);
        assert!(
            core_geometry::diagnose(&pressed).is_closed(),
            "a relief comes out of the field as one closed surface"
        );
    }

    #[test]
    fn a_face_the_map_does_not_cover_stays_where_it_was() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let holed = UvMap::new(vec![None; mesh.faces.len()])
            .without(&[])
            .flipping(&[]);
        let Err(error) = press(
            &mesh,
            &bvh,
            &holed,
            &[flat(1.0)],
            &ReliefSettings::default(),
        ) else {
            panic!("a map that covers no face has nothing to press");
        };
        assert!(matches!(error, VolumeError::UnmappedMesh { mapped: 0, .. }));

        // One face mapped is enough to run, and the rest of the cube keeps its size.
        let mut corners = vec![None; mesh.faces.len()];
        corners[2] = Some(Mapping::sole([Vec2::ZERO; 3]));
        let pressed = press(
            &mesh,
            &bvh,
            &UvMap::new(corners),
            &[flat(1.0)],
            &ReliefSettings {
                amplitude_mm: 0.4,
                ..ReliefSettings::default()
            },
        )
        .expect("one mapped face is a map")
        .mesh;
        let bounds = bounds(&pressed);
        assert!(
            (bounds.mins.z).abs() < TOLERANCE_MM,
            "the unmapped underside stayed on the plate, got {}",
            bounds.mins.z
        );
    }

    #[test]
    fn each_face_is_pressed_from_its_own_image() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        // The lid off the first image, which is white; everything else off the second,
        // which is black.
        let faces = (0..mesh.faces.len())
            .map(|face| {
                Some(Mapping {
                    texture: usize::from(!(2..4).contains(&face)),
                    corners: [Vec2::ZERO; 3],
                })
            })
            .collect();

        let pressed = press(
            &mesh,
            &bvh,
            &UvMap::new(faces),
            &[flat(1.0), flat(0.0)],
            &ReliefSettings {
                amplitude_mm: 0.4,
                ..ReliefSettings::default()
            },
        )
        .expect("two images are two images")
        .mesh;

        let grown = bounds(&pressed);
        assert!(
            (grown.maxs.z - 10.4).abs() < TOLERANCE_MM && grown.mins.z.abs() < TOLERANCE_MM,
            "got {grown:?}, expected only the lid to have moved"
        );
    }

    #[test]
    fn a_map_naming_an_image_that_was_not_given_is_refused() {
        let (mesh, uvs) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let second = UvMap::new(
            (0..mesh.faces.len())
                .map(|face| {
                    uvs.of_face(face).map(|mapping| Mapping {
                        texture: 1,
                        ..mapping
                    })
                })
                .collect(),
        );
        assert!(matches!(
            press(
                &mesh,
                &bvh,
                &second,
                &[flat(1.0)],
                &ReliefSettings::default()
            ),
            Err(VolumeError::MissingTexture {
                wanted: 2,
                given: 1
            })
        ));
    }

    #[test]
    fn an_amplitude_of_nothing_is_refused() {
        let (mesh, uvs) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let settings = ReliefSettings {
            amplitude_mm: 0.0,
            ..ReliefSettings::default()
        };
        assert!(matches!(
            press(&mesh, &bvh, &uvs, &[flat(1.0)], &settings),
            Err(VolumeError::BadAmplitude(0.0))
        ));
    }

    #[test]
    fn a_map_that_does_not_cover_the_mesh_is_refused() {
        let (mesh, _) = cube(10.0);
        let bvh = Bvh::build(&mesh);
        let short = UvMap::whole(vec![[Vec2::ZERO; 3]]);
        assert!(matches!(
            press(
                &mesh,
                &bvh,
                &short,
                &[flat(1.0)],
                &ReliefSettings::default()
            ),
            Err(VolumeError::UnmappedMesh {
                faces: 12,
                mapped: 1
            })
        ));
    }

    #[test]
    fn an_empty_mesh_has_nothing_to_press_into() {
        let mesh = Mesh::default();
        let bvh = Bvh::build(&mesh);
        assert!(matches!(
            press(
                &mesh,
                &bvh,
                &UvMap::default(),
                &[flat(1.0)],
                &ReliefSettings::default()
            ),
            Err(VolumeError::EmptyMesh)
        ));
    }
}
