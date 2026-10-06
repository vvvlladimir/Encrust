use std::io::Cursor;

use core_engine::project::{ProjectError, read_from};
use core_engine::{EngineError, Opening, Run, open_plate};

/// Why a project could not be turned into a sliced file.
#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error(transparent)]
    Project(#[from] ProjectError),

    #[error(transparent)]
    Engine(#[from] EngineError),
}

/// A sliced file, and the extension its container is named with.
#[derive(Debug)]
pub struct Sliced {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
}

/// Slices the active plate of the project in `project` into the container it names.
///
/// The project carries its own geometry, so nothing here is built again: `raster_window`
/// layers are drawn at once, one per thread there is to draw them on.
pub fn slice_project(
    project: &[u8],
    raster_window: usize,
    created_unix_s: u64,
) -> Result<Sliced, WebError> {
    let project = read_from(Cursor::new(project))?;
    let plate = open_plate(
        &project,
        &Opening {
            plate: project.manifest.active_plate,
            raster_window,
            created_unix_s,
        },
    )?;
    let extension = plate.format.extension();
    let run = Run::of(&plate)?;
    drop(plate);
    drop(project);

    let mut sink = Cursor::new(Vec::new());
    run.write("plate", &mut sink, &mut ())?;
    Ok(Sliced {
        bytes: sink.into_inner(),
        extension,
    })
}
