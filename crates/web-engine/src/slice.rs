use std::io::Cursor;

use core_engine::project::{ProjectError, read_from};
use core_engine::{EngineError, Machine, Opening, Run, open_plate};

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

/// The printer and resin a project is sliced for, as the TOML they are kept as: the
/// project names them by id only, and a browser has no catalogue of the user's (ADR 0261).
#[derive(Debug, Clone, Copy)]
pub struct Profiles<'a> {
    pub printer: &'a str,
    pub resin: &'a str,
}

/// Slices the active plate of `project` into the container it names, on the printer and resin in
/// `profiles`, drawing `raster_window` layers at once.
pub fn slice_project(
    project: &[u8],
    profiles: Profiles<'_>,
    raster_window: usize,
    created_unix_s: u64,
) -> Result<Sliced, WebError> {
    let machine = Machine::from_toml(profiles.printer, profiles.resin)?;
    let project = read_from(Cursor::new(project))?;
    let plate = open_plate(
        &project,
        &machine,
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
