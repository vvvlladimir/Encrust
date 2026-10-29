//! A sliced file opened as a model: its layers built back into a mesh, which then stands on
//! the plate like any other and can be cut again (ADR 0294).

use std::sync::Arc;

use anyhow::{Context, Result};
use core_format::OpenFile;
use core_geometry::{Mesh, Orientation, Transform, Vec3, diagnose};
use core_pipeline::file_layout;
use core_volume::{Cancel, Panel, StackField, extract};
use printer_profiles::{Catalogue, Mirror, PrinterProfile};

use crate::files::Handed;
use crate::i18n::tr;
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

/// Highest a layer may stand, millimetres: well over any resin printer's build volume.
const TALLEST_MM: f32 = 1000.0;

/// How a file's panel lies over the plate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub panel: Panel,
    /// The file lays the plate out mirrored along X or Y (ADR 0366), which is undone.
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
    let panel_px = (facts.width_px, facts.height_px);
    let printer = catalogue
        .printer_for_panel(facts.width_px, facts.height_px, facts.machine.as_deref())
        .map(|id| catalogue.printer(id))
        .transpose()?
        .map(|entry| &entry.profile)
        .or(in_hand.filter(|printer| oriented(printer, panel_px).is_some()));
    place(panel_px, facts.display_mm, printer, facts.format)
}

fn place(
    panel_px: (u32, u32),
    display_mm: Option<(f32, f32)>,
    printer: Option<&PrinterProfile>,
    format: &str,
) -> Result<Placement> {
    let stated = display_mm.filter(|&(width, height)| {
        width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0
    });
    let layout = printer.map_or(file_layout(format, Mirror::Normal, ""), |printer| {
        file_layout(format, printer.mirror, printer.machine_name())
    });
    let (size_mm, turned) = printer
        .and_then(|printer| oriented(printer, panel_px))
        .or(stated.map(|size_mm| (size_mm, false)))
        .context(tr!("sliced-no-panel"))?;
    Ok(Placement {
        panel: Panel {
            width_px: panel_px.0,
            height_px: panel_px.1,
            pitch_mm: (size_mm.0 / panel_px.0 as f32, size_mm.1 / panel_px.1 as f32),
        },
        mirror_x: if turned {
            layout.mirror_y
        } else {
            layout.mirror_x
        },
        mirror_y: if turned {
            layout.mirror_x
        } else {
            layout.mirror_y
        },
    })
}

/// The printer's panel size along the file's axes, and whether the file states it portrait
/// where the printer has it landscape or the other way; `None` where it is another panel.
fn oriented(printer: &PrinterProfile, panel_px: (u32, u32)) -> Option<((f32, f32), bool)> {
    let display = &printer.display;
    if (display.width_px, display.height_px) == panel_px {
        Some(((display.width_mm, display.height_mm), false))
    } else if (display.height_px, display.width_px) == panel_px {
        Some(((display.height_mm, display.width_mm), true))
    } else {
        None
    }
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
        .with_context(|| tr!("error-cannot-open", path = name.clone()))?;
    let facts = opened.facts().clone();
    if facts.layer_count() == 0 {
        anyhow::bail!(tr!("sliced-no-layers", name = name));
    }
    let voxel_mm = facts.layer_height_mm.max(FINEST_VOXEL_MM);
    let mut field = StackField::new(placement.panel, voxel_mm);
    let mut bottom_mm = 0.0;
    for (index, entry) in facts.layers.iter().enumerate() {
        let top_mm =
            layer_top_mm(entry.z_mm, bottom_mm, facts.layer_height_mm).with_context(|| {
                tr!(
                    "sliced-layer-unreachable",
                    layer = index + 1,
                    name = name.clone()
                )
            })?;
        let runs: Vec<(u32, u8)> = opened
            .layer(index as u32)
            .with_context(|| {
                tr!(
                    "sliced-cannot-decode",
                    layer = index + 1,
                    name = name.clone()
                )
            })?
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
        anyhow::bail!(tr!("sliced-no-pixel", name = name));
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

/// The top of a layer from the height the file states, or one layer above `bottom_mm` where
/// it states none; `None` past any build volume, which would have the field grow without end.
fn layer_top_mm(z_mm: f32, bottom_mm: f32, layer_height_mm: f32) -> Option<f32> {
    let top_mm = if z_mm > bottom_mm {
        z_mm
    } else {
        bottom_mm + layer_height_mm
    };
    (top_mm.is_finite() && top_mm <= TALLEST_MM).then_some(top_mm)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn printer(panel_px: (u32, u32), size_mm: (f32, f32), mirror_x: bool) -> PrinterProfile {
        let mut printer = PrinterProfile::default();
        printer.display.width_px = panel_px.0;
        printer.display.height_px = panel_px.1;
        printer.display.width_mm = size_mm.0;
        printer.display.height_mm = size_mm.1;
        printer.mirror = if mirror_x {
            Mirror::LcdMirror
        } else {
            Mirror::Normal
        };
        printer
    }

    #[test]
    fn a_file_stating_its_panel_portrait_takes_the_printer_turned() {
        let landscape = printer((200, 100), (40.0, 30.0), true);
        let placed =
            place((100, 200), None, Some(&landscape), "goo").expect("the printer places it");
        assert_eq!(
            (placed.panel.pitch_mm, placed.mirror_x, placed.mirror_y),
            ((0.3, 0.2), true, true),
            "30 mm over 100 px across and 40 mm over 200 px down; a half turn either way"
        );
    }

    #[test]
    fn a_file_is_turned_back_as_its_container_laid_it_out() {
        let mirrored = printer((200, 100), (40.0, 20.0), true);
        let normal = printer((200, 100), (40.0, 20.0), false);
        for (printer, format, undone) in [
            (&mirrored, "goo", (true, true)),
            (&normal, "goo", (false, true)),
            (&mirrored, "photons", (false, true)),
            (&normal, "cxline", (false, false)),
        ] {
            let placed = place((200, 100), None, Some(printer), format).expect("it places");
            assert_eq!(
                (placed.mirror_x, placed.mirror_y),
                undone,
                "{format} on {:?}",
                printer.mirror
            );
        }
    }

    #[test]
    fn a_panel_size_of_nothing_is_not_a_size() {
        for stated in [
            (0.0, 10.0),
            (-5.0, 10.0),
            (f32::NAN, 10.0),
            (f32::INFINITY, 10.0),
        ] {
            assert!(
                place((10, 10), Some(stated), None, "goo").is_err(),
                "{stated:?} gives no pixel pitch to build on"
            );
        }
        let placed =
            place((10, 20), Some((5.0, 4.0)), None, "goo").expect("a stated size places it");
        assert_eq!(
            placed.panel.pitch_mm,
            (0.5, 0.2),
            "5 mm over 10 px, 4 mm over 20 px"
        );
    }

    #[test]
    fn a_layer_past_any_build_volume_is_refused() {
        assert_eq!(
            layer_top_mm(0.0, 1.0, 0.05),
            Some(1.05),
            "no height stated: one layer up"
        );
        assert_eq!(
            layer_top_mm(2.0, 1.0, 0.05),
            Some(2.0),
            "a stated height is kept"
        );
        for z_mm in [1e9, f32::INFINITY] {
            assert_eq!(
                layer_top_mm(z_mm, 1.0, 0.05),
                None,
                "{z_mm} mm is past any printer"
            );
        }
        assert_eq!(layer_top_mm(f32::NAN, 1.0, f32::INFINITY), None);
    }
}
