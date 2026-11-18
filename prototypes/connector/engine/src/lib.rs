//! The connector spike's engine: an STL in, the orientation search with its finalists printed
//! on the automatic supports out, as `--prepare` begins. Measures whether a page with no
//! cross-origin isolation, one thread, does it in time. Not product code.

use std::io::Cursor;
use std::path::Path;

use core_engine::{Machine, SupportPlan, Trial, orient_for_print};
use core_geometry::{DEFAULT_WELD_TOLERANCE, Vec3, weld};
use core_mesh_io::{MeshLoader, ModelFile, StlLoader};
use core_plate::{OrientPreset, OrientSettings, Printed, Watch};
use printer_profiles::Catalogue;
use serde_json::json;
use wasm_bindgen::prelude::*;

/// Prepares the STL `stl` on the printer and resin given as their TOML texts and answers with
/// the best pose's printout as JSON, the way a tool's report reaches the model.
#[wasm_bindgen]
pub fn prepare(stl: &[u8], printer: &str, resin: &str) -> Result<String, JsError> {
    prepared(stl, printer, resin).map_err(|error| JsError::new(&error))
}

fn prepared(stl: &[u8], printer: &str, resin: &str) -> Result<String, String> {
    let machine = Machine::from_toml(printer, resin).map_err(|error| error.to_string())?;
    let mut source = Cursor::new(stl);
    let file = ModelFile {
        path: Path::new("model.stl"),
        source: &mut source,
        beside: &|_| None,
    };
    let loaded = StlLoader.read(file).map_err(|error| error.to_string())?;
    let mesh = weld(&loaded.mesh, DEFAULT_WELD_TOLERANCE).mesh;

    let catalogue = Catalogue::bundled().map_err(|error| error.to_string())?;
    let auto = catalogue
        .autos()
        .next()
        .ok_or("no automatic profile")?
        .profile
        .clone();
    let plan = SupportPlan::sized(auto, &catalogue).map_err(|error| error.to_string())?;
    let placing = plan.placing();
    let volume = machine.printer.build_volume;
    let settings = OrientSettings {
        preset: OrientPreset::default(),
        max_overhang_deg: placing.max_overhang_deg,
        contact_spacing_mm: placing.contact_spacing_mm,
        on_supports: true,
        lift_mm: placing.lift_mm(),
        rest_on_plate: true,
        build_volume_mm: Some(Vec3::new(volume.x, volume.y, volume.z)),
        ..OrientSettings::default()
    };
    let trial = Trial {
        printer: &machine.printer,
        material: &machine.resin,
        support: &plan,
        raster_window: 1,
        keep_clear: None,
    };
    let ranked = orient_for_print(&Printed::solid(&mesh), &settings, Some(&trial), Watch::never())
        .map_err(|error| error.to_string())?;
    let best = ranked.first().ok_or("no pose found")?;
    let printout = best.printout.as_ref();
    Ok(json!({
        "faces": mesh.faces.len(),
        "poses_ranked": ranked.len(),
        "poses_printed": ranked.iter().filter(|choice| choice.printout.is_some()).count(),
        "best": printout.map(|printed| json!({
            "contacts": printed.contacts,
            "hanging": printed.hanging,
            "supports_ml": printed.supports_ml,
            "layers": printed.layers,
            "islands": printed.islands,
            "cups": printed.cups,
            "pull_n": printed.pull_n,
            "resin_ml": printed.resin_ml,
            "overflows": printed.overflows,
        })),
    })
    .to_string())
}

/// Bytes of linear memory the module holds: after a run, the run's peak.
#[wasm_bindgen(js_name = memoryBytes)]
pub fn memory_bytes() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        (core::arch::wasm32::memory_size(0) * 65_536) as f64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0.0
    }
}
