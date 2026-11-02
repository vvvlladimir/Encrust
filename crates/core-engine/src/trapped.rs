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
/// it kept (ADR 0072). `each` hears the layers read and the total after every window; answering
/// false stops the scan, and the answer is then `None`.
pub fn trapped_in(
    baked: &Baked,
    layer_height_mm: Scalar,
    mut each: impl FnMut(usize, usize) -> bool,
) -> Result<Option<Vec<Trapped>>, EngineError> {
    let Some(bounds) = baked.solid.aabb() else {
        return Ok(Some(Vec::new()));
    };
    let windows = cut(baked, &Cutting::uniform(layer_height_mm))?;
    let mut scan = TrapScan::new(
        bounds.mins.truncate(),
        bounds.maxs.truncate(),
        layer_height_mm,
    );
    let total = windows.layer_count();
    let mut read = 0;
    let streamed = windows.stream(&baked.solid, |sliced| {
        for layer in &sliced.layers {
            scan.push(layer);
        }
        read += sliced.layers.len();
        if each(read, total) {
            Ok(())
        } else {
            Err(Halt::Asked)
        }
    });
    match streamed {
        Ok(()) => Ok(Some(scan.finish())),
        Err(Halt::Asked) => Ok(None),
        Err(Halt::Slice(error)) => Err(EngineError::Plan(error)),
    }
}
