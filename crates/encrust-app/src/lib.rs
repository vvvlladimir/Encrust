//! The Encrust window. `main.rs` reads the one argument and starts the logger, and
//! `encrust-web` hands it a canvas in a browser; the window itself, and every tool in it,
//! is here.

mod app;
mod arrange;
mod camera;
mod cut;
mod drain;
mod files;
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
mod repair;
mod report;
mod scene;
mod settings;
mod shortcuts;
mod sliced;
mod slicing;
mod state;
mod status;
mod supports;
mod tool_settings;
#[cfg(target_os = "macos")]
mod traffic_lights;
mod ui;
mod undo;
mod updates;
mod viewport_input;
#[cfg(target_arch = "wasm32")]
mod web;
mod workspace;

use crate::app::SlicerApp;

// What a test needs to put a model on the plate and slice it the way the window does.
pub use files::Handed;
pub use import::prepare;
pub use plate::BuildPlate;
pub use scene::Scene;
pub use slicing::Slicing;
pub use status::Status;

/// Opens the window, with `initial_model` loaded onto the plate if one was named.
#[cfg(not(target_arch = "wasm32"))]
pub fn run(initial_model: Option<&std::path::Path>) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: viewport()?,
        renderer: eframe::Renderer::Wgpu,
        // The viewport draws into its own depth and stencil planes and egui needs none,
        // but the copy into egui's pass must agree with it on samples. See `render::target`.
        multisampling: render::MULTISAMPLING,
        ..Default::default()
    };

    eframe::run_native(
        "Encrust",
        options,
        Box::new(move |cc| Ok(Box::new(SlicerApp::new(cc, initial_model)))),
    )
    .map_err(|e| anyhow::anyhow!("cannot start the window: {e}"))
}

/// Opens the window on `canvas`. `bindings` is the URL of the module's JavaScript, which
/// every worker the window starts imports again; see ADR 0181.
#[cfg(target_arch = "wasm32")]
pub async fn run_web(
    canvas: web_sys::HtmlCanvasElement,
    bindings: String,
) -> Result<(), wasm_bindgen::JsValue> {
    web::thread::init(bindings);
    eframe::WebRunner::new()
        .start(
            canvas,
            eframe::WebOptions::default(),
            Box::new(|cc| Ok(Box::new(SlicerApp::new(cc, None)))),
        )
        .await
}

/// The icon is compiled in rather than read from disk, like the faces in `ui::fonts`.
/// macOS never masks an icon itself, so it gets the squircled art; see `docs/decisions/0135`.
#[cfg(target_os = "macos")]
const ICON: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/icon/encrust-macos-512.png"
));

#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
const ICON: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/icon/encrust-256.png"
));

/// The top bar is the window's own title bar: macOS keeps its buttons over a
/// full-size content view, every other platform draws none. See `docs/decisions/0104`.
#[cfg(not(target_arch = "wasm32"))]
fn viewport() -> anyhow::Result<egui::ViewportBuilder> {
    use anyhow::Context as _;
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
#[cfg(not(target_arch = "wasm32"))]
fn icon() -> anyhow::Result<egui::IconData> {
    use anyhow::Context as _;
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
