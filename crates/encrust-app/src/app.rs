use std::path::Path;

use crate::files::{Arrived, Handed, Wanted};
use crate::panels::Window;
use crate::prefs::Preferences;
use crate::preview::stack_fingerprint;
use crate::project;
use crate::render;
use crate::repair;
use crate::shortcuts;
use crate::state::{Doc, Machine, Tools, View};
use crate::status::Status;
use crate::tool_settings::ToolSettings;
use crate::ui::theme;
use crate::undo::Frame;
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
    /// The keys this frame asked for, read off the raw input before egui saw it.
    keys: Vec<shortcuts::Action>,
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
            keys: Vec::new(),
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
        #[cfg(target_arch = "wasm32")]
        crate::web::hold_context(&cc.egui_ctx);
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
            open_by_what_it_is(&mut app.window(), Handed::Path(path.to_path_buf()));
        }
        app
    }

    /// A sliced file is opened to look at, not imported; anything else is a model.
    fn open_model(&mut self, file: Handed) {
        self.doc
            .imports
            .open(file, &self.doc.plate, &mut self.machine.status);
    }

    /// Opens a sliced file to look at, which is the Preview mode showing a container rather
    /// than the plate. Only its tables are read; a layer is decoded when it is shown.
    fn open_sliced_file(&mut self, file: &Handed) {
        crate::sliced::open(
            &mut self.machine.preview,
            &mut self.mode,
            &mut self.machine.status,
            file,
        );
    }

    /// The machine, the resin, the container and every tool value are written the frame
    /// they change, so the next run opens on them however this one ends. Only on a
    /// settled frame: a slider under the pointer is one choice, not one per frame.
    fn remember_choices(&mut self, settled: bool) {
        if !settled {
            return;
        }
        let now = Preferences::of(
            &self.machine.slicing,
            &self.machine.network,
            &self.machine.updates.prefs,
            &self.tools,
        );
        if now != self.prefs {
            self.prefs = now;
            self.prefs.save();
        }
    }

    /// A file dropped on the window opens the way the dialog would open it: a mesh goes on
    /// to the plate, and a sliced file goes under the layer slider. A browser drops bytes,
    /// several at once for a model and its textures.
    fn take_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input_mut(|input| std::mem::take(&mut input.raw.dropped_files));
        #[cfg(not(target_arch = "wasm32"))]
        for file in dropped {
            open_by_what_it_is(&mut self.window(), Handed::Path(file.path().to_path_buf()));
        }
        // A browser reads a dropped file by a promise, so it arrives on a later frame.
        #[cfg(target_arch = "wasm32")]
        crate::web::files::read_dropped(dropped);
    }

    /// What a browser's dialogs handed over since the last frame, each where it was asked
    /// for.
    fn take_arrived_files(&mut self) {
        for arrived in crate::files::arrived() {
            match arrived {
                Arrived::File(Wanted::Model, file) => self.open_model(file),
                Arrived::File(Wanted::ModelOrProject, file) => {
                    open_by_what_it_is(&mut self.window(), file);
                }
                Arrived::File(Wanted::SlicedFile, file) => self.open_sliced_file(&file),
                Arrived::File(Wanted::Project, file) => {
                    project::open(&mut self.window(), &file);
                }
                Arrived::File(Wanted::PrinterProfile, file) => {
                    crate::profiles::load_printer(&mut self.window(), &file);
                }
                Arrived::File(Wanted::ResinProfile, file) => crate::profiles::load_material(
                    &mut self.machine.slicing,
                    &mut self.machine.status,
                    &file,
                ),
                Arrived::Dropped(file) => open_by_what_it_is(&mut self.window(), file),
                Arrived::Failed(message) => self.machine.status = Status::Error(message),
            }
        }
    }

    /// A stack written for a printer goes straight on to it; one the user named is
    /// already where they asked for it, or in a browser, already handed over as a download.
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

    /// Does what the keys of this frame asked for. They were read off the raw input
    /// before egui's pass, in `shortcuts::take`.
    fn take_shortcuts(&mut self) {
        let fired = std::mem::take(&mut self.keys);
        if fired.is_empty() {
            return;
        }
        let mut window = self.window();
        for action in fired {
            shortcuts::act(&mut window, action);
        }
    }

    /// One frame of the prepare mode's transport: the cut climbing the model while the
    /// section rail is playing. Answers whether it has to keep going.
    fn run_the_cut_up(&mut self, ctx: &egui::Context) -> bool {
        if self.mode != Mode::Prepare {
            self.view.section.playing = false;
            return false;
        }
        crate::panels::section::animate_section(
            ctx,
            &mut self.view.section,
            &self.doc,
            self.machine.slicing.layer_height_mm(),
        )
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

    /// Looks for resin the plate cannot let out whenever a cavity or a cut has moved under
    /// the last check, so that a hollow model says where a hole is needed without being
    /// asked, and says whether a check is now running; see ADR 0189.
    fn check_what_the_plate_traps(&mut self) -> bool {
        if self.tools.hollow.take_hollowed() {
            self.tools.drain.ask_for_a_check();
        }
        // Resin with no way out is inside the model, so the viewport opens the model up
        // to show where. Closing it again is the user's, and it stays closed.
        if self.tools.drain.take_appeared() {
            self.view.options.xray = true;
        }
        let layer_height_mm = self.machine.slicing.layer_height_mm();
        self.tools
            .drain
            .start_if_asked(&self.doc.scene, layer_height_mm)
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

/// Opens a file as whatever it turns out to be: a project onto the plate, a sliced file
/// to look at, or a model. What a drop and the one Open button both go through.
pub fn open_by_what_it_is(window: &mut Window, file: Handed) {
    if crate::files::has_extension(file.path(), &crate::files::PROJECTS) {
        project::open(window, &file);
    } else if core_pipeline::reads_sliced_file(file.path()) {
        crate::sliced::open(
            &mut window.machine.preview,
            window.mode,
            &mut window.machine.status,
            &file,
        );
    } else {
        window
            .doc
            .imports
            .open(file, &window.doc.plate, &mut window.machine.status);
    }
}

impl eframe::App for SlicerApp {
    /// The window reads its own keys here, before egui does: see `shortcuts::take`.
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        self.keys.extend(shortcuts::take(ctx, raw_input));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.take_dropped_files(ui.ctx());
        self.take_arrived_files();

        // A background job reports over a channel, which wakes nothing on its own, so the
        // window has to keep asking for frames while one is running.
        let working = self.doc.imports.poll(
            &mut self.doc.scene,
            &self.doc.plate,
            &mut self.view.camera,
            &mut self.doc.repairs,
            &mut self.machine.status,
        ) | self
            .doc
            .repairs
            .poll(&mut self.doc.scene, &mut self.machine.status)
            | self
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
        // Started after the polls and counted with them, so the frame a run ends on is
        // also the frame its drainage check starts and asks for the next one.
        if working | self.check_what_the_plate_traps() | self.run_the_cut_up(ui.ctx()) {
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
        self.take_shortcuts();

        project::guard_close(ui, &mut self.window());
        repair::ask(
            ui,
            &mut self.doc.repairs,
            &self.doc.scene,
            &mut self.machine.status,
        );
        self.window().show(ui);

        // A gesture is one edit, so nothing is recorded until the button is up again and
        // the field being typed into has been left.
        let frame = Frame {
            settled: ui.ctx().input(|input| !input.pointer.any_down()),
            typing: ui.ctx().memory(|memory| memory.focused().is_some()),
        };
        self.doc.history.observe(
            &self.doc.scene,
            &ToolSettings::of(&self.tools, &self.machine.slicing),
            self.machine.slicing.chosen(),
            frame,
        );
        self.remember_choices(frame.settled && !frame.typing);
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
        app.open_model(Handed::Path("does-not-exist.stl".into()));

        // The read is on a worker thread, so the failure lands on a later frame.
        while app.doc.imports.poll(
            &mut app.doc.scene,
            &app.doc.plate,
            &mut app.view.camera,
            &mut app.doc.repairs,
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
        app.open_sliced_file(&Handed::Path(path));

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
        app.open_model(Handed::Path(fixture));

        while app.doc.imports.poll(
            &mut app.doc.scene,
            &app.doc.plate,
            &mut app.view.camera,
            &mut app.doc.repairs,
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
