//! The Encrust window. `main.rs` reads the one argument and starts the logger; the
//! window itself, and every tool in it, is here.

mod app;
mod arrange;
mod camera;
mod cut;
mod drain;
mod gizmo;
mod hollow;
mod import;
mod job;
mod measure;
mod network;
mod orient;
mod panels;
mod pick;
mod plate;
mod prefs;
mod preview;
mod profiles;
mod project;
mod relief;
mod render;
mod scene;
mod settings;
mod shortcuts;
mod sliced;
mod slicing;
mod state;
mod status;
mod supports;
mod ui;
mod undo;
mod updates;
mod viewport_input;
mod workspace;

use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::app::SlicerApp;

// What a test needs to put a model on the plate and slice it the way the window does.
pub use import::prepare;
pub use plate::BuildPlate;
pub use scene::Scene;
pub use slicing::Slicing;
pub use status::Status;

/// Opens the window, with `initial_model` loaded onto the plate if one was named.
pub fn run(initial_model: Option<&Path>) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: viewport()?,
        renderer: eframe::Renderer::Wgpu,
        // The viewport draws into egui's own render pass, so egui is the one that has to
        // allocate the depth and stencil buffers it needs. See `render::gpu`.
        depth_buffer: render::DEPTH_BUFFER_BITS,
        stencil_buffer: render::STENCIL_BUFFER_BITS,
        multisampling: render::MULTISAMPLING,
        ..Default::default()
    };

    eframe::run_native(
        "Encrust",
        options,
        Box::new(move |cc| Ok(Box::new(SlicerApp::new(cc, initial_model)))),
    )
    .map_err(|e| anyhow!("cannot start the window: {e}"))
}

/// The icon is compiled in rather than read from disk, like the faces in `ui::fonts`.
/// macOS never masks an icon itself, so it gets the squircled art; see `docs/decisions/0135`.
#[cfg(target_os = "macos")]
const ICON: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/icon/encrust-macos-512.png"
));

#[cfg(not(target_os = "macos"))]
const ICON: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/icon/encrust-256.png"
));

/// The title strip is the window's own title bar: macOS keeps its buttons over a
/// full-size content view, every other platform draws none. See `docs/decisions/0104`.
fn viewport() -> Result<egui::ViewportBuilder> {
    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1280.0, 800.0])
        .with_icon(icon().context("the window icon")?);
    Ok(if cfg!(target_os = "macos") {
        viewport
            .with_fullsize_content_view(true)
            .with_titlebar_shown(false)
            .with_title_shown(false)
    } else {
        viewport.with_decorations(false)
    })
}

/// The window icon on Windows and Linux; on macOS `eframe` also hands it to the Dock,
/// over whatever the bundle's `.icns` set.
fn icon() -> Result<egui::IconData> {
    let image = image::load_from_memory(ICON)
        .context("cannot decode the compiled-in icon")?
        .into_rgba8();
    let (width, height) = image.dimensions();
    Ok(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    })
}
