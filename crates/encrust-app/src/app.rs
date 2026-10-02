use std::path::Path;

use crate::panels::Window;
use crate::prefs::Preferences;
use crate::preview::stack_fingerprint;
use crate::project;
use crate::render;
use crate::shortcuts;
use crate::state::{Doc, Machine, Tools, View};
use crate::status::Status;
use crate::ui::theme;
use crate::workspace::{Mode, Tool};

/// Window state: what the user is doing, the scene being edited, and the camera looking
/// at it. The layout that draws it is fixed; see `crate::panels`.
pub struct SlicerApp {
    mode: Mode,
    /// The mode the last frame was drawn in, so a switch is noticed wherever it came from.
    drawn_mode: Mode,
    tool: Tool,
    doc: Doc,
    view: View,
    tools: Tools,
    machine: Machine,
    /// What the last run was set to, so a change to it is noticed and written once.
    prefs: Preferences,
}

impl Default for SlicerApp {
    fn default() -> Self {
        let doc = Doc::default();
        Self {
            mode: Mode::default(),
            drawn_mode: Mode::default(),
            tool: Tool::default(),
            view: View::framing(&doc.plate),
            doc,
            tools: Tools::default(),
            machine: Machine::default(),
            prefs: Preferences::default(),
        }
    }
}

impl SlicerApp {
    /// Builds the window, puts the viewport's GPU resources where egui keeps them, dresses
    /// the context in the Encrust tokens, and imports the model named on the command line.
    pub fn new(cc: &eframe::CreationContext<'_>, initial_model: Option<&Path>) -> Self {
        if let Some(render_state) = cc.wgpu_render_state.as_ref() {
            render::install(render_state);
        }
        theme::apply(&cc.egui_ctx);
        crate::updates::sweep();

        let mut app = Self::default();
        app.machine
            .slicing
            .load_user_profiles(&mut app.machine.status);
        app.prefs = crate::prefs::load();
        let prefs = app.prefs.clone();
        prefs.apply(&mut app.window());
        project::mark_saved(&mut app.window());
        if let Some(path) = initial_model {
            // A sliced file named on the command line is opened to look at, not imported.
            if core_pipeline::reads_sliced_file(path) {
                app.open_sliced_file(path);
            } else {
                app.open_model(path);
            }
        }
        app
    }

    fn open_model(&mut self, path: &Path) {
        self.doc.imports.open(
            path.to_path_buf(),
            &self.doc.plate,
            &mut self.machine.status,
        );
    }

    /// Opens a sliced file to look at, which is the Preview mode showing a container rather
    /// than the plate. Only its tables are read; a layer is decoded when it is shown.
    fn open_sliced_file(&mut self, path: &Path) {
        crate::sliced::open(
            &mut self.machine.preview,
            &mut self.mode,
            &mut self.machine.status,
            path,
        );
    }

    /// The machine, the resin and the container are written the frame they change, so
    /// the next run opens on them however this one ends.
    fn remember_choices(&mut self) {
        let now = Preferences::of(
            &self.machine.slicing,
            &self.machine.network,
            &self.machine.updates.prefs,
        );
        if now != self.prefs {
            self.prefs = now;
            self.prefs.save();
        }
    }

    /// A file dropped on the window opens the way the dialog would open it: a mesh goes on
    /// to the plate, and a sliced file goes under the layer slider.
    fn take_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        for path in dropped {
            if core_pipeline::reads_sliced_file(&path) {
                self.open_sliced_file(&path);
            } else {
                self.open_model(&path);
            }
        }
    }

    /// A stack written for a printer goes straight on to it; one the user named is
    /// already where they asked for it.
    fn hand_written_file_to_the_network(&mut self) {
        let Some(written) = self.machine.slicing.take_written() else {
            return;
        };
        if written.to_send {
            self.machine
                .network
                .send(written.path, &mut self.machine.status);
        }
    }

    /// The preview shows the plate as it is, so entering the mode with a stale stack, or
    /// none, cuts one. Nothing to cut, or nothing to cut it for, and it stays empty: the
    /// transport says which.
    fn refresh_preview(&mut self) {
        if self.mode != Mode::Preview || self.machine.preview.is_building() {
            return;
        }
        // A file being shown is the file, not the plate: nothing about the scene makes it
        // stale and there is no stack of ours to measure.
        if self.machine.preview.read_facts().is_some() {
            return;
        }
        let fingerprint = stack_fingerprint(&self.doc.scene, self.machine.slicing.cutting());
        let wanted =
            self.machine.preview.layer_count() == 0 || self.machine.preview.is_stale(fingerprint);
        let Some(settings) = self.machine.slicing.raster_settings() else {
            return;
        };
        if !wanted {
            // Preview says what the print takes, so the stack is measured as it is written.
            self.machine
                .preview
                .measure(&settings, self.machine.slicing.fold());
            return;
        }
        if !self.doc.scene.has_printable(self.doc.scene.active_plate()) {
            return;
        }

        let started = self
            .machine
            .preview
            .build(&self.doc.scene, self.machine.slicing.cutting());
        if let Err(error) = started {
            self.machine.status = Status::failed(&error.context("cannot build the layer preview"));
        }
    }

    /// Both modes cut the model at one height, so a switch hands it over: Preview parks on
    /// the layer nearest where Prepare was cut, and Prepare takes the layer Preview showed.
    fn carry_height(&mut self) {
        if self.mode == self.drawn_mode {
            return;
        }
        match self.mode {
            Mode::Preview => self
                .machine
                .preview
                .show_height(self.view.section.height_mm),
            Mode::Prepare => {
                if let Some(height_mm) = self.machine.preview.prepare_height() {
                    self.view.section.height_mm = height_mm;
                }
            }
        }
        self.drawn_mode = self.mode;
    }

    /// Answers the keyboard, unless a field has the cursor: Ctrl+Z inside a number is
    /// the number's own undo. Every key comes from `crate::shortcuts`.
    ///
    /// The test is the text cursor rather than `egui_wants_keyboard_input`, which is true
    /// of any widget merely holding focus — a button clicked a minute ago.
    fn take_shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.text_edit_focused() {
            return;
        }
        let fired = shortcuts::pressed(ctx);
        if fired.is_empty() {
            return;
        }
        let mut window = self.window();
        for action in fired {
            shortcuts::act(&mut window, action);
        }
    }

    /// Every part of the window, borrowed at once, which is what a panel is drawn with.
    fn window(&mut self) -> Window<'_> {
        Window {
            mode: &mut self.mode,
            tool: &mut self.tool,
            doc: &mut self.doc,
            view: &mut self.view,
            tools: &mut self.tools,
            machine: &mut self.machine,
        }
    }

    /// Rebuilds the columns and the painted patches of every object whose placement,
    /// points, paint or profile moved under them, so that what the viewport draws is what
    /// would be sliced.
    fn refresh_supports(&mut self) {
        let table = self.tools.supports.table();
        for object in self.doc.scene.objects_mut() {
            let untouched = object.supports.is_empty()
                && object.supports.meshes().is_none()
                && object.supports.painted().is_empty()
                && object.supports.blocked().is_empty();
            if untouched {
                continue;
            }
            let mesh = std::sync::Arc::clone(&object.mesh);
            let bvh = std::sync::Arc::clone(&object.bvh);
            object
                .supports
                .refresh(&mesh, &bvh, object.transform, &table);
        }
    }
}

impl eframe::App for SlicerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.take_dropped_files(ui.ctx());

        // A background job reports over a channel, which wakes nothing on its own, so the
        // window has to keep asking for frames while one is running.
        let working = self.doc.imports.poll(
            &mut self.doc.scene,
            &self.doc.plate,
            &mut self.view.camera,
            &mut self.machine.status,
        ) | self
            .machine
            .slicing
            .poll(&self.doc.scene, &mut self.machine.status)
            | self.machine.network.poll(&mut self.machine.status)
            | self.machine.preview.poll(&mut self.machine.status)
            | self.machine.updates.poll(&mut self.machine.status)
            | self
                .tools
                .supports
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .tools
                .hollow
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .tools
                .relief
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .tools
                .drain
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
                .tools
                .orient
                .poll(&mut self.doc.scene, &mut self.machine.status)
            | crate::panels::animate_preview(ui.ctx(), &mut self.machine.preview);
        if working {
            ui.ctx().request_repaint();
        }
        self.machine.updates.tick();
        if self.machine.updates.take_close() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        self.hand_written_file_to_the_network();
        self.refresh_preview();
        self.carry_height();
        self.refresh_supports();
        self.take_shortcuts(ui.ctx());

        project::guard_close(ui, &mut self.window());
        self.window().show(ui);

        // A gesture is one edit, so nothing is recorded until the button is up again.
        let settled = ui.ctx().input(|input| !input.pointer.any_down());
        self.doc.history.observe(&self.doc.scene, settled);
        self.remember_choices();
    }

    fn on_exit(&mut self) {
        self.machine.updates.on_exit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_opens_on_the_prepare_mode_with_nothing_selected() {
        let app = SlicerApp::default();
        assert_eq!(app.mode, Mode::Prepare);
        assert_eq!(app.tool, Tool::Select);
        assert!(
            app.view.options.grid,
            "the plate grid is on until it is turned off"
        );
    }

    #[test]
    fn the_opening_camera_looks_at_the_plate() {
        let app = SlicerApp::default();
        assert_eq!(app.view.camera.target, app.doc.plate.center());
        assert!(app.doc.scene.is_empty());
    }

    #[test]
    fn a_file_that_cannot_be_read_leaves_the_scene_alone() {
        let mut app = SlicerApp::default();
        app.open_model(Path::new("does-not-exist.stl"));

        // The read is on a worker thread, so the failure lands on a later frame.
        while app.doc.imports.poll(
            &mut app.doc.scene,
            &app.doc.plate,
            &mut app.view.camera,
            &mut app.machine.status,
        ) {
            std::thread::yield_now();
        }

        assert!(app.doc.scene.is_empty());
        assert!(app.machine.status.is_error());
    }

    #[test]
    fn a_sliced_file_named_on_the_command_line_goes_under_the_layer_slider() {
        // Written by the window's own path, so nothing but the opening is under test; the
        // containers themselves are checked in `core-pipeline/tests/read_back.rs`.
        let path = crate::preview::tests::written_goo("app-startup", 3);
        let mut app = SlicerApp::default();
        app.open_sliced_file(&path);

        assert_eq!(
            app.mode,
            Mode::Preview,
            "a file opens in the mode that shows it"
        );
        let facts = app
            .machine
            .preview
            .read_facts()
            .expect("the file is under the slider");
        assert_eq!(facts.format, "goo");
        assert_eq!(facts.layer_count(), 3);
        assert!(app.doc.scene.is_empty(), "nothing reached the plate");
    }

    #[test]
    fn a_file_no_reader_claims_is_opened_as_a_model() {
        assert!(!core_pipeline::reads_sliced_file(Path::new("cube.stl")));
        assert!(core_pipeline::reads_sliced_file(Path::new("plate.ctb")));
    }

    #[test]
    fn a_model_opened_from_the_command_line_reaches_the_plate() {
        let fixture =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cube.stl");
        let mut app = SlicerApp::default();
        app.open_model(&fixture);

        while app.doc.imports.poll(
            &mut app.doc.scene,
            &app.doc.plate,
            &mut app.view.camera,
            &mut app.machine.status,
        ) {
            std::thread::yield_now();
        }

        assert_eq!(app.doc.scene.objects().len(), 1);
        assert!(!app.machine.status.is_error(), "a sound cube opens");
    }

    #[test]
    fn tab_switches_between_preparing_and_previewing() {
        let mut app = SlicerApp::default();
        shortcuts::act(&mut app.window(), shortcuts::Action::ToggleMode);
        assert_eq!(app.mode, Mode::Preview);
        shortcuts::act(&mut app.window(), shortcuts::Action::ToggleMode);
        assert_eq!(app.mode, Mode::Prepare);
    }

    /// The sheet is modal: a key pressed under it moves nothing on the plate.
    #[test]
    fn the_sheet_holds_the_keyboard_until_it_is_closed() {
        let mut app = SlicerApp::default();
        shortcuts::act(&mut app.window(), shortcuts::Action::Sheet);
        assert!(app.view.options.sheet);

        shortcuts::act(&mut app.window(), shortcuts::Action::Pick(Tool::Hollow));
        assert_eq!(
            app.tool,
            Tool::Select,
            "the tool is not reached under the sheet"
        );
        assert!(app.view.options.sheet);

        shortcuts::act(&mut app.window(), shortcuts::Action::Deselect);
        assert!(!app.view.options.sheet, "Escape closes it");
    }

    #[test]
    fn a_switch_back_to_prepare_without_a_stack_keeps_its_own_height() {
        let mut app = SlicerApp::default();
        app.view.section.height_mm = Some(3.5);
        app.mode = Mode::Preview;
        app.carry_height();
        app.mode = Mode::Prepare;
        app.carry_height();
        assert_eq!(app.view.section.height_mm, Some(3.5));
    }

    #[test]
    fn the_preview_is_not_built_without_a_printer_profile() {
        let mut app = SlicerApp {
            mode: Mode::Preview,
            ..SlicerApp::default()
        };
        app.refresh_preview();
        assert!(!app.machine.preview.is_building());
        assert!(
            !app.machine.status.is_error(),
            "an empty plate is not a failure"
        );
    }
}
