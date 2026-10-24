//! The Encrust window. `main.rs` reads the one argument and starts the logger, and
//! `encrust-web` hands it a canvas in a browser; the window itself, and every tool in it,
//! is here.

mod app;
mod arrange;
mod boolean;
mod camera;
mod cut;
mod drain;
mod drilling;
mod exchange;
mod files;
mod gizmo;
mod growth;
mod hollow;
mod i18n;
mod import;
mod job;
mod landing;
mod launch;
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
mod recent;
mod relief;
mod render;
mod repair;
mod report;
mod samples;
mod scene;
mod settings;
mod shortcuts;
mod sliced;
mod slicing;
mod state;
mod status;
mod supports;
mod surfaces;
mod text;
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

// What a test needs to put a model on the plate and slice it the way the window does.
pub use files::Handed;
pub use import::prepare;
pub use plate::BuildPlate;
pub use scene::Scene;
pub use slicing::Slicing;
pub use status::Status;

#[cfg(not(target_arch = "wasm32"))]
pub use launch::run;
#[cfg(target_arch = "wasm32")]
pub use launch::run_web;
