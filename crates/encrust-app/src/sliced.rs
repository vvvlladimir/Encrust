//! A sliced file opened as a model: its layers built back into a mesh, which then stands on
//! the plate like any other and can be cut again (ADR 0294).

use std::sync::Arc;

use anyhow::{Context, Result};
use core_format::{OpenFile, SlicedFile};
use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
use core_volume::{Cancel, Panel, StackField, extract};
use printer_profiles::{Catalogue, PrinterProfile};

use crate::files::Handed;
use crate::job::ImportStage;
use crate::scene::{ImportSummary, Imported};

/// The containers a reader claims, which open as a model would.
pub const EXTENSIONS: [&str; 14] = [
    "goo", "ctb", "cbddlp", "photon", "pwmx", "pwmo", "pwms", "pws", "sl1", "sl1s", "zip", "cxdlp",
    "svgx", "cws",
];

/// Finest lattice a file is built back on, millimetres: a few pixels of any panel sold, and
/// fine enough that a miniature's features survive.
const FINEST_VOXEL_MM: f32 = 0.05;

/// How a file's panel lies over the plate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub panel: Panel,
    /// The panel mirrors along X or Y, so the file's masks are the model mirrored.
    pub mirror_x: bool,
    pub mirror_y: bool,
}

/// Where the file's panel lies, from the machine it was cut for: the one of its panel in the
/// catalogue, or else `in_hand` where that is the same panel; the file's own size otherwise.
pub fn placement(
    file: &Handed,
    catalogue: &Catalogue,
    in_hand: Option<&PrinterProfile>,
) -> Result<Placement> {
    let opened = core_pipeline::open(file.path(), file.reader()?)?;
    let facts = opened.facts();
    let printer = catalogue
        .printer_for_panel(facts.width_px, facts.height_px, facts.machine.as_deref())
        .and_then(|id| catalogue.printer(id).ok())
        .map(|entry| &entry.profile)
        .or(in_hand.filter(|printer| same_panel(printer, facts)));
    let size_mm = printer
        .map(|printer| (printer.display.width_mm, printer.display.height_mm))
        .or(facts.display_mm)
        .context("the file states no panel size and no printer of its panel is known; choose the printer it was cut for first")?;
    Ok(Placement {
        panel: Panel {
            width_px: facts.width_px,
            height_px: facts.height_px,
            pitch_mm: (
                size_mm.0 / facts.width_px as f32,
                size_mm.1 / facts.height_px as f32,
            ),
        },
        mirror_x: printer.is_some_and(|printer| printer.mirror_x),
        mirror_y: printer.is_some_and(|printer| printer.mirror_y),
    })
}

fn same_panel(printer: &PrinterProfile, facts: &SlicedFile) -> bool {
    let display = &printer.display;
    (display.width_px, display.height_px) == (facts.width_px, facts.height_px)
}

/// Builds the file's layers back into a model, named after the file and standing where its
/// pixels stood. `stage` is called before each part of the work starts.
pub fn prepare(
    file: &Handed,
    placement: Placement,
    stage: &mut dyn FnMut(ImportStage),
) -> Result<Imported> {
    let name = file.path().file_name().map_or_else(
        || file.path().display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    stage(ImportStage::Reading);
    let mut opened = core_pipeline::open(file.path(), file.reader()?)
        .with_context(|| format!("cannot open {name}"))?;
    let facts = opened.facts().clone();
    if facts.layer_count() == 0 {
        anyhow::bail!("{name} has no layers");
    }
    let voxel_mm = facts.layer_height_mm.max(FINEST_VOXEL_MM);
    let mut field = StackField::new(placement.panel, voxel_mm);
    let mut bottom_mm = 0.0;
    for (index, entry) in facts.layers.iter().enumerate() {
        let top_mm = match entry.z_mm {
            z if z > bottom_mm => z,
            _ => bottom_mm + facts.layer_height_mm,
        };
        let runs: Vec<(u32, u8)> = opened
            .layer(index as u32)
            .with_context(|| format!("cannot decode layer {} of {name}", index + 1))?
            .iter()
            .map(|run| (run.length, run.value))
            .collect();
        field.push(bottom_mm, top_mm, &runs);
        bottom_mm = top_mm;
    }

    stage(ImportStage::Repairing);
    let mut mesh = extract(&field.finish(), Cancel::never());
    unmirror(&mut mesh, placement);
    if mesh.faces.is_empty() {
        anyhow::bail!("{name} lights no pixel");
    }
    stage(ImportStage::Indexing);
    let summary = ImportSummary {
        vertices_merged: 0,
        faces_removed: 0,
        orientation: Orientation {
            flipped_faces: 0,
            inverted_shells: 0,
            orientable: true,
        },
        diagnostics: diagnose(&mesh),
    };
    Ok(Imported::new(
        name,
        Arc::new(mesh),
        Transform::default(),
        summary,
    ))
}

/// Takes the panel's own mirroring back out, so the model is the one the file was cut from.
fn unmirror(mesh: &mut Mesh, placement: Placement) {
    let panel = placement.panel;
    let span = Vec3::new(
        panel.width_px as f32 * panel.pitch_mm.0,
        panel.height_px as f32 * panel.pitch_mm.1,
        0.0,
    );
    for vertex in &mut mesh.vertices {
        if placement.mirror_x {
            vertex.x = span.x - vertex.x;
        }
        if placement.mirror_y {
            vertex.y = span.y - vertex.y;
        }
    }
    // One reflection turns every face inside out; two put it back.
    if placement.mirror_x != placement.mirror_y {
        for face in &mut mesh.faces {
            face.swap(1, 2);
        }
    }
}
