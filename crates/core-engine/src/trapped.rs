use core_geometry::Scalar;
use core_slicer::SliceError;
use core_supports::{TrapScan, Trapped};

use crate::bake::Baked;
use crate::cut::{Cutting, cut};
use crate::error::EngineError;

/// Why the layers stopped coming before the last.
enum Halt {
    Slice(SliceError),
    Asked,
}

impl From<SliceError> for Halt {
    fn from(error: SliceError) -> Self {
        Self::Slice(error)
    }
}

/// The pockets of resin with no way out of `baked`, cut `layer_height_mm` a layer and none of
/// it kept (ADR 0072). `each` hears the layers read and the total once before any is cut and
/// again before each window is read; answering false stops the scan, and the answer is `None`.
pub fn trapped_in(
    baked: &Baked,
    layer_height_mm: Scalar,
    mut each: impl FnMut(usize, usize) -> bool,
) -> Result<Option<Vec<Trapped>>, EngineError> {
    let Some(bounds) = baked.solid.aabb() else {
        return Ok(Some(Vec::new()));
    };
    let windows = cut(baked, &Cutting::uniform(layer_height_mm))?;
    let total = windows.layer_count();
    if !each(0, total) {
        return Ok(None);
    }
    let mut scan = TrapScan::new(
        bounds.mins.truncate(),
        bounds.maxs.truncate(),
        layer_height_mm,
    );
    let mut read = 0;
    let streamed = windows.stream(&baked.solid, |sliced| {
        if !each(read, total) {
            return Err(Halt::Asked);
        }
        for layer in &sliced.layers {
            scan.push(layer);
        }
        read += sliced.layers.len();
        Ok(())
    });
    match streamed {
        Ok(()) => Ok(Some(scan.finish())),
        Err(Halt::Asked) => Ok(None),
        Err(Halt::Slice(error)) => Err(EngineError::Plan(error)),
    }
}

#[cfg(test)]
mod tests {
    use core_geometry::{Vec3, cuboid};

    use super::*;

    fn cube() -> Baked {
        Baked {
            solid: cuboid(Vec3::ZERO, Vec3::splat(10.0)).into(),
            ceiling_mm: 10.0,
            floor_mm: 0.0,
        }
    }

    #[test]
    fn the_total_is_heard_and_a_stop_honoured_before_any_layer_is_read() {
        let mut heard = Vec::new();
        let found = trapped_in(&cube(), 0.5, |read, total| {
            heard.push((read, total));
            false
        })
        .expect("a cube cuts");
        assert!(found.is_none(), "the scan stopped");
        assert_eq!(
            heard,
            vec![(0, 20)],
            "10 mm in 0.5 mm layers, none read yet"
        );
    }

    #[test]
    fn a_stop_between_windows_comes_before_the_next_is_read() {
        let mut heard = Vec::new();
        let found = trapped_in(&cube(), 0.05, |read, total| {
            heard.push((read, total));
            heard.len() < 3
        })
        .expect("a cube cuts");
        assert!(found.is_none(), "the scan stopped");
        // 200 layers in windows of 64: asked before any, before the first and before the second.
        assert_eq!(heard, vec![(0, 200), (0, 200), (64, 200)]);
    }

    #[test]
    fn a_solid_with_no_faces_is_refused_before_any_layer_is_heard() {
        let points = core_geometry::Mesh::new(vec![Vec3::ZERO, Vec3::splat(10.0)], Vec::new());
        let baked = Baked {
            solid: points.into(),
            ceiling_mm: 10.0,
            floor_mm: 0.0,
        };
        let mut heard = 0;
        let error = trapped_in(&baked, 0.5, |_, _| {
            heard += 1;
            true
        })
        .expect_err("no faces, no layers");
        assert!(
            matches!(error, EngineError::Plan(SliceError::EmptyMesh)),
            "got {error:?}"
        );
        assert_eq!(heard, 0, "the error comes before the total");
    }
}
